//! The yt-dlp CLI facade: metadata parsing, playlist listing and downloads.

use std::fmt;
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use regex::Regex;

use crate::cancellation::CancellationToken;
use crate::models::{MediaInfo, PlaylistInfo};
use crate::services::ffmpeg_path_service::FfmpegPathService;
use crate::services::media_info_parser::{MediaInfoParser, ParseError};
use crate::services::process_runner::{LineCallback, ProcessError, ProcessResult, ProcessRunner};
use crate::services::ytdlp_path_service::YtDlpPathService;

/// Message shown when yt-dlp could not be located.
pub const NOT_CONFIGURED_MESSAGE: &str = "未找到 yt-dlp。请到“设置”页选择 yt-dlp.exe 的路径。";

static PERCENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[download\]\s+(\d+(?:\.\d+)?)%").unwrap());
static DESTINATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[download\]\s+Destination:\s+(.+?)\s*$").unwrap());
static SPEED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[download\].*?\bat\s+(.+?)\s+ETA\b").unwrap());
static ETA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bETA\s+(\S+)").unwrap());
static MERGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"Merging formats into\s+"(.+?)"\s*$"#).unwrap());

/// One incremental update reported while a download runs.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadUpdate {
    /// A plain yt-dlp log line when it carried no structured progress.
    pub line: Option<String>,
    /// Download percentage, clamped to `0..=100`.
    pub percent: Option<f64>,
    /// Destination or merged output path, when the line announced one.
    pub output_path: Option<String>,
    pub speed: Option<String>,
    pub eta: Option<String>,
}

/// Final result of a download attempt.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadOutcome {
    pub success: bool,
    pub cancelled: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

impl DownloadOutcome {
    fn failed(message: impl Into<String>) -> Self {
        Self {
            success: false,
            cancelled: false,
            output_path: None,
            error: Some(message.into()),
        }
    }

    fn cancelled() -> Self {
        Self {
            success: false,
            cancelled: true,
            output_path: None,
            error: None,
        }
    }
}

/// A parsed URL: either a single video or a playlist (video queue).
#[derive(Debug, Clone, PartialEq)]
pub struct MediaParseResult {
    pub media: Option<MediaInfo>,
    pub playlist: Option<PlaylistInfo>,
}

/// Optional callback receiving incremental [`DownloadUpdate`]s.
pub type ProgressCallback = Arc<dyn Fn(DownloadUpdate) + Send + Sync + 'static>;

/// Error returned by the CLI facade.
#[derive(Debug)]
pub enum YtDlpError {
    /// The operation was cancelled.
    Cancelled,
    /// yt-dlp reported a failure, or the output could not be parsed.
    Message(String),
    /// The process could not be started or supervised.
    Io(std::io::Error),
}

impl fmt::Display for YtDlpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            YtDlpError::Cancelled => write!(f, "操作已取消"),
            YtDlpError::Message(message) => write!(f, "{message}"),
            YtDlpError::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for YtDlpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            YtDlpError::Io(error) => Some(error),
            _ => None,
        }
    }
}

/// Everything needed for one download invocation.
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub format_expression: String,
    pub output_directory: String,
    pub cookie_file: Option<String>,
    pub proxy_url: Option<String>,
    pub concurrent_fragments: i32,
    pub output_template: Option<String>,
    pub write_thumbnail: bool,
    pub live: bool,
}

impl DownloadRequest {
    pub fn new(
        url: impl Into<String>,
        format_expression: impl Into<String>,
        output_directory: impl Into<String>,
    ) -> Self {
        Self {
            url: url.into(),
            format_expression: format_expression.into(),
            output_directory: output_directory.into(),
            cookie_file: None,
            proxy_url: None,
            concurrent_fragments: 1,
            output_template: None,
            write_thumbnail: false,
            live: false,
        }
    }
}

impl Default for DownloadRequest {
    fn default() -> Self {
        Self::new(String::new(), String::new(), String::new())
    }
}

/// Tracks the last announced destination and the final merged path.
#[derive(Default)]
struct DownloadTracking {
    last_destination: Option<String>,
    final_path: Option<String>,
}

/// Thin facade over the `yt-dlp` executable.
pub struct YtDlpCli {
    runner: Arc<dyn ProcessRunner>,
    paths: Arc<dyn YtDlpPathService>,
    ffmpeg_paths: Arc<dyn FfmpegPathService>,
    parser: Arc<dyn MediaInfoParser>,
}

impl YtDlpCli {
    pub fn new(
        runner: Arc<dyn ProcessRunner>,
        paths: Arc<dyn YtDlpPathService>,
        ffmpeg_paths: Arc<dyn FfmpegPathService>,
        parser: Arc<dyn MediaInfoParser>,
    ) -> Self {
        Self {
            runner,
            paths,
            ffmpeg_paths,
            parser,
        }
    }

    pub fn resolve_executable(&self) -> Option<String> {
        self.paths.resolve()
    }

    /// Runs `yt-dlp --version`; returns `None` when yt-dlp is missing or fails.
    pub async fn get_version(
        &self,
        cancellation: CancellationToken,
    ) -> Result<Option<String>, YtDlpError> {
        let Some(executable) = self.paths.resolve() else {
            return Ok(None);
        };

        let arguments = vec!["--version".to_string()];
        let result = match self
            .run_process(&executable, &arguments, None, None, cancellation)
            .await
        {
            Ok(result) => result,
            Err(YtDlpError::Cancelled) => return Ok(None),
            Err(error) => return Err(error),
        };

        if result.exit_code == 0 {
            Ok(Some(result.standard_output.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    pub async fn get_media_info(
        &self,
        url: &str,
        cookie_file: Option<&str>,
        proxy_url: Option<&str>,
        cancellation: CancellationToken,
    ) -> Result<MediaInfo, YtDlpError> {
        let Some(executable) = self.paths.resolve() else {
            return Err(YtDlpError::Message(NOT_CONFIGURED_MESSAGE.to_string()));
        };

        let mut arguments = vec![
            "--encoding",
            "utf-8",
            "--skip-download",
            "--no-playlist",
            "--no-warnings",
            "--dump-single-json",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        add_cookie_arguments(&mut arguments, cookie_file);
        add_proxy_arguments(&mut arguments, proxy_url);
        arguments.push(url.to_string());

        let result = self
            .run_process(&executable, &arguments, None, None, cancellation)
            .await?;

        if result.cancelled {
            return Err(YtDlpError::Cancelled);
        }

        if result.exit_code != 0 {
            return Err(YtDlpError::Message(last_meaningful(
                &result.standard_error,
                &result.standard_output,
            )));
        }

        self.parser
            .parse(&result.standard_output)
            .map_err(|error| YtDlpError::Message(format!("解析视频信息失败：{error}")))
    }

    /// Parses a URL that may point at a single video or a playlist (video queue).
    /// Exactly one of the two result fields is populated.
    pub async fn get_media_or_playlist(
        &self,
        url: &str,
        cookie_file: Option<&str>,
        proxy_url: Option<&str>,
        cancellation: CancellationToken,
    ) -> Result<MediaParseResult, YtDlpError> {
        let Some(executable) = self.paths.resolve() else {
            return Err(YtDlpError::Message(NOT_CONFIGURED_MESSAGE.to_string()));
        };

        // --flat-playlist lists the videos of a playlist without fetching each
        // one, while a plain video URL is still resolved with its full format list.
        let mut arguments = vec![
            "--encoding",
            "utf-8",
            "--skip-download",
            "--flat-playlist",
            "--no-warnings",
            "--dump-single-json",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
        add_cookie_arguments(&mut arguments, cookie_file);
        add_proxy_arguments(&mut arguments, proxy_url);
        arguments.push(url.to_string());

        let result = self
            .run_process(&executable, &arguments, None, None, cancellation.clone())
            .await?;

        if result.cancelled {
            return Err(YtDlpError::Cancelled);
        }

        if result.exit_code != 0 {
            return Err(YtDlpError::Message(last_meaningful(
                &result.standard_error,
                &result.standard_output,
            )));
        }

        let parsed = match self.parser.parse_playlist(&result.standard_output) {
            Ok(Some(playlist)) => Ok(MediaParseResult {
                media: None,
                playlist: Some(playlist),
            }),
            Ok(None) => self
                .parser
                .parse(&result.standard_output)
                .map(|media| MediaParseResult {
                    media: Some(media),
                    playlist: None,
                }),
            Err(error) => Err(error),
        };

        match parsed {
            Ok(parsed) => Ok(parsed),
            // A flat listing of a single video can omit formats; re-query it as a
            // concrete video instead of failing.
            Err(ParseError::InvalidData(_)) => {
                let media = self
                    .get_media_info(url, cookie_file, proxy_url, cancellation)
                    .await?;
                Ok(MediaParseResult {
                    media: Some(media),
                    playlist: None,
                })
            }
            Err(error) => Err(YtDlpError::Message(format!("解析视频信息失败：{error}"))),
        }
    }

    pub async fn download(
        &self,
        request: DownloadRequest,
        progress: Option<ProgressCallback>,
        cancellation: CancellationToken,
    ) -> Result<DownloadOutcome, YtDlpError> {
        let Some(executable) = self.paths.resolve() else {
            return Ok(DownloadOutcome::failed(NOT_CONFIGURED_MESSAGE));
        };

        if let Err(error) = std::fs::create_dir_all(&request.output_directory) {
            return Ok(DownloadOutcome::failed(format!(
                "无法创建输出目录：{error}"
            )));
        }

        let name_template = match request.output_template.as_deref().map(str::trim) {
            Some(template) if !template.is_empty() => template.to_string(),
            _ => "%(title)s.%(ext)s".to_string(),
        };
        let template = Path::new(&request.output_directory).join(&name_template);

        let mut arguments = vec![
            "--encoding".to_string(),
            "utf-8".to_string(),
            "--no-playlist".to_string(),
            "--newline".to_string(),
            "--no-warnings".to_string(),
            "-f".to_string(),
            request.format_expression.clone(),
            "-o".to_string(),
            template.to_string_lossy().into_owned(),
        ];
        add_cookie_arguments(&mut arguments, request.cookie_file.as_deref());
        add_proxy_arguments(&mut arguments, request.proxy_url.as_deref());

        // Only pass an explicit location when ffmpeg is not already on PATH.
        if let Some(ffmpeg) = self.ffmpeg_paths.explicit_path_for_yt_dlp() {
            arguments.push("--ffmpeg-location".to_string());
            arguments.push(ffmpeg);
        }

        if request.concurrent_fragments > 1 {
            arguments.push("--concurrent-fragments".to_string());
            arguments.push(request.concurrent_fragments.to_string());
        }

        // Save the cover as a separate image file. Deliberately avoid
        // --embed-thumbnail, which would merge the cover into the media.
        if request.write_thumbnail {
            arguments.push("--write-thumbnail".to_string());
        }

        // Live stream: capture from the beginning of the stream, and use the
        // MPEG-TS container so an interrupted capture keeps what was recorded.
        if request.live {
            arguments.push("--live-from-start".to_string());
            arguments.push("--hls-use-mpegts".to_string());
        }

        arguments.push(request.url.clone());

        let tracking = Arc::new(Mutex::new(DownloadTracking::default()));
        let on_line = build_line_handler(Arc::clone(&tracking), progress);

        let result = match self
            .run_process(
                &executable,
                &arguments,
                Some(Arc::clone(&on_line)),
                Some(on_line),
                cancellation,
            )
            .await
        {
            Ok(result) => result,
            Err(YtDlpError::Cancelled) => return Ok(DownloadOutcome::cancelled()),
            Err(error) => return Err(error),
        };

        if result.cancelled {
            return Ok(DownloadOutcome::cancelled());
        }

        if result.exit_code != 0 {
            return Ok(DownloadOutcome::failed(last_meaningful(
                &result.standard_error,
                &result.standard_output,
            )));
        }

        let output_path = tracking.lock().ok().and_then(|guard| {
            guard
                .final_path
                .clone()
                .or_else(|| guard.last_destination.clone())
        });

        Ok(DownloadOutcome {
            success: true,
            cancelled: false,
            output_path,
            error: None,
        })
    }

    async fn run_process(
        &self,
        executable: &str,
        arguments: &[String],
        on_stdout: Option<LineCallback>,
        on_stderr: Option<LineCallback>,
        cancellation: CancellationToken,
    ) -> Result<ProcessResult, YtDlpError> {
        match self
            .runner
            .run(executable, arguments, on_stdout, on_stderr, cancellation)
            .await
        {
            Ok(result) => Ok(result),
            Err(ProcessError::Cancelled) => Err(YtDlpError::Cancelled),
            Err(ProcessError::Io(error)) => Err(YtDlpError::Io(error)),
        }
    }
}

fn build_line_handler(
    tracking: Arc<Mutex<DownloadTracking>>,
    progress: Option<ProgressCallback>,
) -> LineCallback {
    Arc::new(move |line: &str| {
        let Some(progress) = progress.as_ref() else {
            return;
        };

        let trimmed = line.trim_end_matches('\r');

        if let Some(value) = PERCENT_RE
            .captures(trimmed)
            .and_then(|captures| captures.get(1))
            .and_then(|group| group.as_str().parse::<f64>().ok())
        {
            let speed = SPEED_RE
                .captures(trimmed)
                .and_then(|captures| captures.get(1))
                .map(|group| group.as_str().trim().to_string());
            let eta = ETA_RE
                .captures(trimmed)
                .and_then(|captures| captures.get(1))
                .map(|group| group.as_str().trim().to_string());

            progress(DownloadUpdate {
                line: None,
                percent: Some(value.clamp(0.0, 100.0)),
                output_path: None,
                speed,
                eta,
            });
            return;
        }

        if let Some(path) = capture_group(&MERGE_RE, trimmed, 1) {
            if let Ok(mut guard) = tracking.lock() {
                guard.last_destination = Some(path.clone());
                guard.final_path = Some(path.clone());
            }
            progress(DownloadUpdate {
                line: None,
                percent: None,
                output_path: Some(path),
                speed: None,
                eta: None,
            });
            return;
        }

        if let Some(path) = capture_group(&DESTINATION_RE, trimmed, 1) {
            if let Ok(mut guard) = tracking.lock() {
                guard.last_destination = Some(path.clone());
            }
            progress(DownloadUpdate {
                line: None,
                percent: None,
                output_path: Some(path),
                speed: None,
                eta: None,
            });
            return;
        }

        progress(DownloadUpdate {
            line: Some(trimmed.to_string()),
            percent: None,
            output_path: None,
            speed: None,
            eta: None,
        });
    })
}

fn capture_group(regex: &Regex, text: &str, index: usize) -> Option<String> {
    regex
        .captures(text)
        .and_then(|captures| captures.get(index))
        .map(|group| group.as_str().trim().to_string())
}

fn add_cookie_arguments(arguments: &mut Vec<String>, cookie_file: Option<&str>) {
    let Some(cookie_file) = cookie_file else {
        return;
    };

    let cookie_file = cookie_file.trim();
    if cookie_file.is_empty() {
        return;
    }

    arguments.push("--cookies".to_string());
    arguments.push(cookie_file.to_string());
}

fn add_proxy_arguments(arguments: &mut Vec<String>, proxy_url: Option<&str>) {
    let Some(proxy_url) = proxy_url else {
        return;
    };

    let proxy_url = proxy_url.trim();
    if proxy_url.is_empty() {
        return;
    }

    arguments.push("--proxy".to_string());
    arguments.push(proxy_url.to_string());
}

/// Joins the last three non-empty lines of stderr + stdout, or a generic message.
pub fn last_meaningful(standard_error: &str, standard_output: &str) -> String {
    let combined = format!("{standard_error}\n{standard_output}");
    let lines = combined
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if lines.is_empty() {
        return "未知错误（yt-dlp 非零退出码）".to_string();
    }

    let start = lines.len().saturating_sub(3);
    lines[start..].join(" | ")
}
