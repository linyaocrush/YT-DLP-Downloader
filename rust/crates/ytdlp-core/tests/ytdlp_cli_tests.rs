//! Ported from `YtDlpDownloader.Core.Tests/Services/YtDlpCliTests.cs`.
//!
//! The Moq mocks are replaced by hand-written fakes for the four collaborator
//! traits.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ytdlp_downloader_core::{
    AudioSource, CancellationToken, DownloadRequest, DownloadUpdate, FfmpegPathService,
    LineCallback, MediaInfo, MediaInfoParser, MediaParseResult, ParseError, PlaylistEntry,
    PlaylistInfo, ProcessError, ProcessResult, ProcessRunner, ProgressCallback, VideoSource,
    YtDlpCli, YtDlpError, YtDlpPathService,
};

mod common;
use common::TempDir;

// ---- Fakes ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum FakeFailure {
    None,
    Cancelled,
}

struct FakeRunner {
    result: ProcessResult,
    stdout_lines: Vec<String>,
    failure: FakeFailure,
    calls: Mutex<Vec<Vec<String>>>,
}

impl FakeRunner {
    fn new(result: ProcessResult) -> Self {
        Self {
            result,
            stdout_lines: Vec::new(),
            failure: FakeFailure::None,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn with_lines(result: ProcessResult, lines: &[&str]) -> Self {
        let mut runner = Self::new(result);
        runner.stdout_lines = lines.iter().map(|line| line.to_string()).collect();
        runner
    }

    fn failing_cancelled() -> Self {
        let mut runner = Self::new(ok_process());
        runner.failure = FakeFailure::Cancelled;
        runner
    }

    fn arguments(&self) -> Vec<String> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .cloned()
            .collect()
    }

    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait]
impl ProcessRunner for FakeRunner {
    async fn run(
        &self,
        _file_name: &str,
        arguments: &[String],
        on_stdout: Option<LineCallback>,
        _on_stderr: Option<LineCallback>,
        _cancellation: CancellationToken,
    ) -> Result<ProcessResult, ProcessError> {
        self.calls.lock().unwrap().push(arguments.to_vec());

        if self.failure == FakeFailure::Cancelled {
            return Err(ProcessError::Cancelled);
        }

        if let Some(callback) = &on_stdout {
            for line in &self.stdout_lines {
                callback(line);
            }
        }

        Ok(self.result.clone())
    }
}

struct FakePaths {
    resolved: Option<String>,
}

impl FakePaths {
    fn new(resolved: Option<&str>) -> Self {
        Self {
            resolved: resolved.map(str::to_string),
        }
    }
}

impl YtDlpPathService for FakePaths {
    fn locate_in_path(&self) -> Option<String> {
        self.resolved.clone()
    }

    fn resolve(&self) -> Option<String> {
        self.resolved.clone()
    }
}

struct FakeFfmpeg {
    explicit: Option<String>,
}

impl FakeFfmpeg {
    fn new(explicit: Option<&str>) -> Self {
        Self {
            explicit: explicit.map(str::to_string),
        }
    }
}

impl FfmpegPathService for FakeFfmpeg {
    fn locate_in_path(&self) -> Option<String> {
        None
    }

    fn configured_path(&self) -> Option<String> {
        self.explicit.clone()
    }

    fn explicit_path_for_yt_dlp(&self) -> Option<String> {
        self.explicit.clone()
    }
}

struct FakeParser {
    playlist: Option<PlaylistInfo>,
    media: MediaInfo,
    parse_outcomes: Mutex<VecDeque<Result<MediaInfo, ParseError>>>,
    parse_calls: AtomicUsize,
    playlist_calls: AtomicUsize,
}

impl FakeParser {
    fn new(media: MediaInfo) -> Self {
        Self {
            playlist: None,
            media,
            parse_outcomes: Mutex::new(VecDeque::new()),
            parse_calls: AtomicUsize::new(0),
            playlist_calls: AtomicUsize::new(0),
        }
    }

    fn with_playlist(mut self, playlist: PlaylistInfo) -> Self {
        self.playlist = Some(playlist);
        self
    }

    fn failing_once(self, error: ParseError) -> Self {
        self.parse_outcomes.lock().unwrap().push_back(Err(error));
        self
    }
}

impl MediaInfoParser for FakeParser {
    fn parse(&self, _json: &str) -> Result<MediaInfo, ParseError> {
        self.parse_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(outcome) = self.parse_outcomes.lock().unwrap().pop_front() {
            return outcome;
        }
        Ok(self.media.clone())
    }

    fn parse_playlist(&self, _json: &str) -> Result<Option<PlaylistInfo>, ParseError> {
        self.playlist_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.playlist.clone())
    }
}

// ---- Helpers -------------------------------------------------------------

fn ok_process() -> ProcessResult {
    ProcessResult {
        cancelled: false,
        exit_code: 0,
        standard_output: String::new(),
        standard_error: String::new(),
    }
}

fn process(exit_code: i32, stdout: &str, stderr: &str) -> ProcessResult {
    ProcessResult {
        cancelled: false,
        exit_code,
        standard_output: stdout.to_string(),
        standard_error: stderr.to_string(),
    }
}

fn cancelled_process() -> ProcessResult {
    ProcessResult {
        cancelled: true,
        exit_code: 0,
        standard_output: String::new(),
        standard_error: String::new(),
    }
}

fn sample_media() -> MediaInfo {
    MediaInfo::new(
        "1",
        "t",
        "u",
        None,
        None,
        None,
        Vec::<VideoSource>::new(),
        Vec::<AudioSource>::new(),
    )
}

fn build_cli(
    runner: &Arc<FakeRunner>,
    paths: &Arc<FakePaths>,
    ffmpeg: &Arc<FakeFfmpeg>,
    parser: &Arc<FakeParser>,
) -> YtDlpCli {
    YtDlpCli::new(
        Arc::clone(runner),
        Arc::clone(paths),
        Arc::clone(ffmpeg),
        Arc::clone(parser),
    )
}

fn has(arguments: &[String], value: &str) -> bool {
    arguments.iter().any(|argument| argument == value)
}

fn recorder() -> (ProgressCallback, Arc<Mutex<Vec<DownloadUpdate>>>) {
    let items = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&items);
    let callback: ProgressCallback = Arc::new(move |update| sink.lock().unwrap().push(update));
    (callback, items)
}

// ---- Tests ---------------------------------------------------------------

#[test]
fn resolve_executable_delegates_to_path_service() {
    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    assert_eq!(cli.resolve_executable().as_deref(), Some("C:/yt-dlp.exe"));
}

#[tokio::test]
async fn get_version_no_executable_returns_null_without_running() {
    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(None));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    assert_eq!(cli.get_version(CancellationToken::new()).await.unwrap(), None);
    assert_eq!(runner.call_count(), 0);
}

#[tokio::test]
async fn get_version_success_returns_trimmed_version() {
    let runner = Arc::new(FakeRunner::new(process(0, "  2024.01.01\n", "")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    assert_eq!(
        cli.get_version(CancellationToken::new()).await.unwrap().as_deref(),
        Some("2024.01.01")
    );
}

#[tokio::test]
async fn get_version_non_zero_exit_returns_null() {
    let runner = Arc::new(FakeRunner::new(process(1, "boom", "")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    assert_eq!(cli.get_version(CancellationToken::new()).await.unwrap(), None);
}

#[tokio::test]
async fn get_media_info_no_executable_throws() {
    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(None));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let error = cli
        .get_media_info("url", None, None, CancellationToken::new())
        .await
        .unwrap_err();

    assert!(matches!(error, YtDlpError::Message(_)));
}

#[tokio::test]
async fn get_media_info_success_passes_arguments_and_parses() {
    let runner = Arc::new(FakeRunner::new(process(0, "{}", "")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let parser = Arc::new(FakeParser::new(sample_media()));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &parser);

    let info = cli
        .get_media_info(
            "https://youtu.be/1",
            Some("c.txt"),
            Some("http://p:1"),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(info, sample_media());
    let arguments = runner.arguments();
    assert!(has(&arguments, "--dump-single-json"));
    assert!(has(&arguments, "--skip-download"));
    assert_eq!(arguments.last().map(String::as_str), Some("https://youtu.be/1"));
    assert!(has(&arguments, "--cookies"));
    assert!(has(&arguments, "c.txt"));
    assert!(has(&arguments, "--proxy"));
    assert!(has(&arguments, "http://p:1"));
}

#[tokio::test]
async fn get_media_info_cancelled_throws_cancelled() {
    let runner = Arc::new(FakeRunner::new(cancelled_process()));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let error = cli
        .get_media_info("url", None, None, CancellationToken::new())
        .await
        .unwrap_err();

    assert!(matches!(error, YtDlpError::Cancelled));
}

#[tokio::test]
async fn get_media_info_non_zero_exit_throws_with_last_meaningful_error() {
    let runner = Arc::new(FakeRunner::new(process(1, "stdout tail", "line1\nline2")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let error = cli
        .get_media_info("url", None, None, CancellationToken::new())
        .await
        .unwrap_err();

    let YtDlpError::Message(message) = error else {
        panic!("expected YtDlpError::Message");
    };
    assert!(message.contains("line2"), "{message}");
    assert!(message.contains("stdout tail"), "{message}");
}

#[tokio::test]
async fn get_media_info_parser_failure_wrapped_in_message() {
    let runner = Arc::new(FakeRunner::new(process(0, "{}", "")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let parser = Arc::new(
        FakeParser::new(sample_media()).failing_once(ParseError::InvalidData("bad data".to_string())),
    );
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &parser);

    let error = cli
        .get_media_info("url", None, None, CancellationToken::new())
        .await
        .unwrap_err();

    let YtDlpError::Message(message) = error else {
        panic!("expected YtDlpError::Message");
    };
    assert!(message.contains("解析视频信息失败"), "{message}");
    assert!(message.contains("bad data"), "{message}");
}

#[tokio::test]
async fn download_no_executable_returns_error_outcome() {
    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(None));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let outcome = cli
        .download(DownloadRequest::new("url", "bv", "C:/out"), None, CancellationToken::new())
        .await
        .unwrap();

    assert!(!outcome.success);
    assert!(!outcome.cancelled);
    assert!(outcome.error.is_some());
}

#[tokio::test]
async fn download_success_builds_expected_arguments() {
    let temp = TempDir::new();
    let output = temp.path().to_string_lossy().into_owned();

    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let ffmpeg = Arc::new(FakeFfmpeg::new(Some("C:/ffmpeg.exe")));
    let cli = build_cli(&runner, &paths, &ffmpeg, &Arc::new(FakeParser::new(sample_media())));

    let mut request = DownloadRequest::new("https://youtu.be/1", "bv*[height<=720]+ba", output.clone());
    request.cookie_file = Some("cookies.txt".to_string());
    request.proxy_url = Some("socks5://p:1080".to_string());
    request.concurrent_fragments = 3;
    request.output_template = Some("%(id)s.%(ext)s".to_string());
    request.write_thumbnail = true;

    let outcome = cli.download(request, None, CancellationToken::new()).await.unwrap();

    assert!(outcome.success);
    let arguments = runner.arguments();
    assert!(has(&arguments, "-f"));
    assert!(has(&arguments, "bv*[height<=720]+ba"));
    assert!(has(&arguments, "--cookies"));
    assert!(has(&arguments, "cookies.txt"));
    assert!(has(&arguments, "--proxy"));
    assert!(has(&arguments, "socks5://p:1080"));
    assert!(has(&arguments, "--ffmpeg-location"));
    assert!(has(&arguments, "C:/ffmpeg.exe"));
    assert!(has(&arguments, "--concurrent-fragments"));
    assert!(has(&arguments, "3"));
    assert!(has(&arguments, "--write-thumbnail"));
    let template = std::path::Path::new(&output)
        .join("%(id)s.%(ext)s")
        .to_string_lossy()
        .into_owned();
    assert!(has(&arguments, &template), "{arguments:?}");
}

#[tokio::test]
async fn download_default_template_and_single_fragment() {
    let temp = TempDir::new();
    let output = temp.path().to_string_lossy().into_owned();

    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    cli.download(DownloadRequest::new("url", "bv", output.clone()), None, CancellationToken::new())
        .await
        .unwrap();

    let arguments = runner.arguments();
    assert!(!has(&arguments, "--concurrent-fragments"));
    assert!(!has(&arguments, "--write-thumbnail"));
    assert!(!has(&arguments, "--cookies"));
    assert!(!has(&arguments, "--proxy"));
    let template = std::path::Path::new(&output)
        .join("%(title)s.%(ext)s")
        .to_string_lossy()
        .into_owned();
    assert!(has(&arguments, &template), "{arguments:?}");
}

#[tokio::test]
async fn download_destination_then_merge_uses_merged_path() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::with_lines(
        ok_process(),
        &[
            "[download] Destination: C:/out/video.f137.mp4",
            "Merging formats into \"C:/out/final.mp4\"",
        ],
    ));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));
    let (progress, _items) = recorder();

    let outcome = cli
        .download(
            DownloadRequest::new("url", "bv", temp.path().to_string_lossy()),
            Some(progress),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert!(outcome.success);
    assert_eq!(outcome.output_path.as_deref(), Some("C:/out/final.mp4"));
}

#[tokio::test]
async fn download_only_destination_uses_destination() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::with_lines(
        ok_process(),
        &["[download] Destination: C:/out/only.mp4"],
    ));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));
    let (progress, _items) = recorder();

    let outcome = cli
        .download(
            DownloadRequest::new("url", "bv", temp.path().to_string_lossy()),
            Some(progress),
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert_eq!(outcome.output_path.as_deref(), Some("C:/out/only.mp4"));
}

#[tokio::test]
async fn download_progress_lines_report_percent_speed_eta() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::with_lines(
        ok_process(),
        &[
            "[download]  50.0% of 10.00MiB at 1.00MiB/s ETA 00:05",
            "[download]  150.0% of 10.00MiB at 2.00MiB/s ETA 00:01",
            "some plain status line",
        ],
    ));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let ffmpeg = Arc::new(FakeFfmpeg::new(None));
    let cli = build_cli(&runner, &paths, &ffmpeg, &Arc::new(FakeParser::new(sample_media())));
    let (progress, items) = recorder();

    cli.download(
        DownloadRequest::new("url", "bv", temp.path().to_string_lossy()),
        Some(progress),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let items = items.lock().unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].percent, Some(50.0));
    assert_eq!(items[0].speed.as_deref(), Some("1.00MiB/s"));
    assert_eq!(items[0].eta.as_deref(), Some("00:05"));
    assert_eq!(items[1].percent, Some(100.0));
    assert_eq!(items[2].line.as_deref(), Some("some plain status line"));
}

#[tokio::test]
async fn download_cancelled_result_returns_cancelled_outcome() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::new(cancelled_process()));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let outcome = cli
        .download(
            DownloadRequest::new("url", "bv", temp.path().to_string_lossy()),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert!(outcome.cancelled);
    assert!(!outcome.success);
}

#[tokio::test]
async fn download_runner_throws_cancelled_returns_cancelled_outcome() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::failing_cancelled());
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let outcome = cli
        .download(
            DownloadRequest::new("url", "bv", temp.path().to_string_lossy()),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert!(outcome.cancelled);
    assert_eq!(outcome.error, None);
}

#[tokio::test]
async fn download_non_zero_exit_returns_error_outcome() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::new(process(2, "out", "ERROR: failed badly")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let outcome = cli
        .download(
            DownloadRequest::new("url", "bv", temp.path().to_string_lossy()),
            None,
            CancellationToken::new(),
        )
        .await
        .unwrap();

    assert!(!outcome.success);
    assert!(!outcome.cancelled);
    assert!(outcome.error.unwrap().contains("failed badly"));
}

#[tokio::test]
async fn get_media_or_playlist_playlist_returns_playlist_and_uses_flat_listing() {
    let runner = Arc::new(FakeRunner::new(process(0, "{}", "")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let playlist = PlaylistInfo {
        id: "PL1".to_string(),
        title: "List".to_string(),
        entries: vec![PlaylistEntry {
            url: "https://example.com/1".to_string(),
            title: "One".to_string(),
        }],
    };
    let parser = Arc::new(FakeParser::new(sample_media()).with_playlist(playlist.clone()));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &parser);

    let result = cli
        .get_media_or_playlist("https://example.com/playlist", None, None, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.playlist, Some(playlist));
    assert_eq!(result.media, None);
    let arguments = runner.arguments();
    assert!(has(&arguments, "--flat-playlist"));
    assert!(has(&arguments, "--dump-single-json"));
    assert_eq!(arguments.last().map(String::as_str), Some("https://example.com/playlist"));
    assert_eq!(parser.parse_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn get_media_or_playlist_single_video_returns_media() {
    let runner = Arc::new(FakeRunner::new(process(0, "{}", "")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let result: MediaParseResult = cli
        .get_media_or_playlist("https://youtu.be/1", None, None, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.media, Some(sample_media()));
    assert_eq!(result.playlist, None);
}

#[tokio::test]
async fn get_media_or_playlist_flat_single_missing_formats_falls_back_to_media_info() {
    let runner = Arc::new(FakeRunner::new(process(0, "{}", "")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let parser = Arc::new(
        FakeParser::new(sample_media())
            .failing_once(ParseError::InvalidData("no formats".to_string())),
    );
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &parser);

    let result = cli
        .get_media_or_playlist("https://youtu.be/1", None, None, CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(result.media, Some(sample_media()));
    assert_eq!(result.playlist, None);
    let arguments = runner.arguments();
    assert!(has(&arguments, "--flat-playlist"));
    assert!(has(&arguments, "--no-playlist"));
}

#[tokio::test]
async fn get_media_or_playlist_non_zero_exit_throws() {
    let runner = Arc::new(FakeRunner::new(process(1, "out", "ERROR: nope")));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let error = cli
        .get_media_or_playlist("url", None, None, CancellationToken::new())
        .await
        .unwrap_err();

    let YtDlpError::Message(message) = error else {
        panic!("expected YtDlpError::Message");
    };
    assert!(message.contains("nope"), "{message}");
}

#[tokio::test]
async fn download_live_adds_live_capture_flags() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    let mut request = DownloadRequest::new("url", "bv", temp.path().to_string_lossy());
    request.live = true;
    cli.download(request, None, CancellationToken::new()).await.unwrap();

    let arguments = runner.arguments();
    assert!(has(&arguments, "--live-from-start"));
    assert!(has(&arguments, "--hls-use-mpegts"));
}

#[tokio::test]
async fn download_not_live_omits_live_capture_flags() {
    let temp = TempDir::new();
    let runner = Arc::new(FakeRunner::new(ok_process()));
    let paths = Arc::new(FakePaths::new(Some("C:/yt-dlp.exe")));
    let cli = build_cli(&runner, &paths, &Arc::new(FakeFfmpeg::new(None)), &Arc::new(FakeParser::new(sample_media())));

    cli.download(
        DownloadRequest::new("url", "bv", temp.path().to_string_lossy()),
        None,
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let arguments = runner.arguments();
    assert!(!has(&arguments, "--live-from-start"));
    assert!(!has(&arguments, "--hls-use-mpegts"));
}