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
    pub phase: Option<String>,
    pub downloaded_bytes: Option<u64>,
    pub elapsed: Option<f64>,
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
    pub save_log: bool,
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
            save_log: false,
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
            "--ignore-no-formats-error",
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
            return Err(YtDlpError::Message(friendly_error(&last_meaningful(
                &result.standard_error,
                &result.standard_output,
            ))));
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
            "--ignore-no-formats-error",
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
            return Err(YtDlpError::Message(friendly_error(&last_meaningful(
                &result.standard_error,
                &result.standard_output,
            ))));
        }

        let playlist_result = self.parser.parse_playlist(&result.standard_output);
        if let Ok(Some(playlist)) = &playlist_result {
            if !playlist.entries.is_empty() {
                return Ok(MediaParseResult {
                    media: None,
                    playlist: Some(playlist.clone()),
                });
            }
        }

        // Some YouTube generated/recommendation lists (for example RDMM) do
        // not produce a useful playlist object with --dump-single-json even
        // though yt-dlp can enumerate every entry with --print url.
        let looks_like_playlist = url.split('?').nth(1).is_some_and(|query| {
            query
                .split('&')
                .any(|part| part.split('=').next() == Some("list"))
        }) || url.to_ascii_lowercase().contains("/playlist");
        if looks_like_playlist
            && (playlist_result.is_err()
                || matches!(&playlist_result, Ok(Some(playlist)) if playlist.entries.is_empty())
                || matches!(&playlist_result, Ok(None)))
        {
            if let Ok(playlist) = self
                .get_flat_playlist(url, cookie_file, proxy_url, cancellation.clone())
                .await
            {
                return Ok(MediaParseResult {
                    media: None,
                    playlist: Some(playlist),
                });
            }
        }

        match self.parser.parse(&result.standard_output) {
            Ok(media) => Ok(MediaParseResult {
                media: Some(media),
                playlist: None,
            }),
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

    async fn get_flat_playlist(
        &self,
        url: &str,
        cookie_file: Option<&str>,
        proxy_url: Option<&str>,
        cancellation: CancellationToken,
    ) -> Result<PlaylistInfo, YtDlpError> {
        let Some(executable) = self.paths.resolve() else {
            return Err(YtDlpError::Message(NOT_CONFIGURED_MESSAGE.to_string()));
        };
        let mut arguments = vec![
            "--encoding".to_string(),
            "utf-8".to_string(),
            "--skip-download".to_string(),
            "--flat-playlist".to_string(),
            "--no-warnings".to_string(),
            "--print".to_string(),
            "%(webpage_url)s\t%(title)s\t%(thumbnail)s".to_string(),
        ];
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
            return Err(YtDlpError::Message(friendly_error(&last_meaningful(
                &result.standard_error,
                &result.standard_output,
            ))));
        }

        let entries = result
            .standard_output
            .lines()
            .map(str::trim)
            .filter_map(|line| {
                let mut fields = line.splitn(3, '\t');
                let url = fields.next()?.trim();
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return None;
                }
                let title = fields.next().unwrap_or(url).trim();
                let thumbnail = fields
                    .next()
                    .map(str::trim)
                    .filter(|value| value.starts_with("http://") || value.starts_with("https://"));
                Some((
                    url,
                    title,
                    thumbnail
                        .map(str::to_string)
                        .or_else(|| youtube_thumbnail_url(url)),
                ))
            })
            .enumerate()
            .map(
                |(index, (url, title, thumbnail))| crate::models::PlaylistEntry {
                    index: index + 1,
                    url: url.to_string(),
                    title: title.to_string(),
                    thumbnail_url: thumbnail,
                },
            )
            .collect::<Vec<_>>();
        if entries.is_empty() {
            return Err(YtDlpError::Message(
                "yt-dlp 未返回播放列表视频链接".to_string(),
            ));
        }
        Ok(PlaylistInfo {
            id: "-".to_string(),
            title: "播放列表".to_string(),
            entries,
        })
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
        if let Err(error) = tempfile::NamedTempFile::new_in(&request.output_directory) {
            return Ok(DownloadOutcome::failed(format!("输出目录不可写：{error}")));
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
            "--progress".to_string(),
            "--progress-template".to_string(),
            "download:__YTDLP_PROGRESS__%(progress)j|||%(info.vcodec)s|||%(info.format_id)s"
                .to_string(),
            "--progress-template".to_string(),
            "postprocess:__YTDLP_POSTPROCESS__%(progress)j".to_string(),
            "--print".to_string(),
            "after_move:__YTDLP_FILE__%(filepath)j".to_string(),
            "--no-simulate".to_string(),
            "--no-overwrites".to_string(),
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
        let handler = build_line_handler(Arc::clone(&tracking), progress);
        let on_line: LineCallback = if request.save_log {
            use std::io::Write;
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let log_path =
                Path::new(&request.output_directory).join(format!("download-{stamp}.log"));
            let file = std::fs::File::create(&log_path).map_err(YtDlpError::Io)?;
            let log = Mutex::new(file);
            let cookie = request.cookie_file.clone();
            Arc::new(move |line: &str| {
                let mut safe = line.to_string();
                if let Some(path) = &cookie {
                    safe = safe.replace(path, "[Cookie 文件已隐藏]");
                }
                let lower = safe.to_lowercase();
                if lower.contains("authorization:") || lower.contains("cookie:") {
                    safe = "[敏感请求头已隐藏]".into();
                }
                if let Ok(mut file) = log.lock() {
                    let _ = writeln!(file, "{safe}");
                }
                handler(line);
            })
        } else {
            handler
        };

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
            return Ok(DownloadOutcome::failed(friendly_error(&last_meaningful(
                &result.standard_error,
                &result.standard_output,
            ))));
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
        let noop = |_: DownloadUpdate| {};
        let progress: &dyn Fn(DownloadUpdate) = progress
            .as_ref()
            .map(|callback| callback.as_ref() as &dyn Fn(DownloadUpdate))
            .unwrap_or(&noop);

        let trimmed = line.trim_end_matches('\r');
        if let Some(json) = trimmed.strip_prefix("__YTDLP_FILE__") {
            if let Ok(path) = serde_json::from_str::<String>(json) {
                if let Ok(mut guard) = tracking.lock() {
                    guard.final_path = Some(path.clone());
                }
                progress(DownloadUpdate {
                    phase: Some("Completed".into()),
                    downloaded_bytes: None,
                    elapsed: None,
                    line: None,
                    percent: Some(100.0),
                    output_path: Some(path),
                    speed: None,
                    eta: None,
                });
                return;
            }
        }
        for (prefix, phase) in [
            ("__YTDLP_PROGRESS__", "Downloading"),
            ("__YTDLP_POSTPROCESS__", "Processing"),
        ] {
            if let Some(json) = trimmed.strip_prefix(prefix) {
                let mut fields = json.splitn(3, "|||");
                let json = fields.next().unwrap_or_default();
                let codec = fields.next();
                let stream_id = fields.next();
                if let Ok(root) = serde_json::from_str::<serde_json::Value>(json) {
                    let value = root.get("progress").unwrap_or(&root);
                    let info = root.get("info").unwrap_or(&value["info_dict"]);
                    let bytes = value["downloaded_bytes"].as_u64();
                    let total = value["total_bytes"]
                        .as_f64()
                        .or_else(|| value["total_bytes_estimate"].as_f64());
                    let percent = bytes
                        .zip(total)
                        .filter(|(_, total)| *total > 0.0)
                        .map(|(bytes, total)| (bytes as f64 / total * 100.0).clamp(0.0, 100.0));
                    let format_id = stream_id
                        .or_else(|| info["format_id"].as_str())
                        .unwrap_or_default();
                    let stage = if phase == "Processing" {
                        "Processing"
                    } else if codec.or_else(|| info["vcodec"].as_str()) == Some("none") {
                        "DownloadingAudio"
                    } else {
                        "DownloadingVideo"
                    };
                    progress(DownloadUpdate {
                        phase: Some(format!("{stage}:{format_id}")),
                        downloaded_bytes: bytes,
                        elapsed: value["elapsed"].as_f64(),
                        line: None,
                        percent,
                        output_path: None,
                        speed: value["speed"]
                            .as_f64()
                            .map(|speed| format!("{:.2} MiB/s", speed / 1048576.0)),
                        eta: value["eta"].as_f64().map(|eta| format!("{}s", eta as u64)),
                    });
                    return;
                }
            }
        }

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
                phase: None,
                downloaded_bytes: None,
                elapsed: None,
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
                phase: Some("Processing".into()),
                downloaded_bytes: None,
                elapsed: None,
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
                phase: None,
                downloaded_bytes: None,
                elapsed: None,
                line: None,
                percent: None,
                output_path: Some(path),
                speed: None,
                eta: None,
            });
            return;
        }

        progress(DownloadUpdate {
            phase: None,
            downloaded_bytes: None,
            elapsed: None,
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

fn youtube_thumbnail_url(url: &str) -> Option<String> {
    let host = url
        .split("//")
        .nth(1)?
        .split('/')
        .next()?
        .split(':')
        .next()?;
    let video_id = if host.eq_ignore_ascii_case("youtu.be") {
        url.split("//")
            .nth(1)?
            .split('/')
            .nth(1)?
            .split(['?', '&'])
            .next()?
    } else if host.ends_with("youtube.com") {
        url.split('?')
            .nth(1)?
            .split('&')
            .find_map(|part| part.strip_prefix("v="))?
    } else {
        return None;
    };
    (!video_id.is_empty()).then(|| format!("https://i.ytimg.com/vi/{video_id}/hqdefault.jpg"))
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
///
/// yt-dlp prints a bare `null` to stdout when `--dump-single-json` fails (the
/// extractor returned no info), so it must be filtered out; otherwise the UI
/// would show `ERROR: ... | null`.
pub fn last_meaningful(standard_error: &str, standard_output: &str) -> String {
    let combined = format!("{standard_error}\n{standard_output}");
    let mut lines = combined
        .split('\n')
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !is_ignorable_output_line(line))
        .collect::<Vec<_>>();
    lines.dedup();

    if lines.is_empty() {
        return "未知错误（yt-dlp 非零退出码）".to_string();
    }

    // Prefer the actual ERROR/WARNING lines over incidental progress output.
    let error_lines = lines
        .iter()
        .copied()
        .filter(|line| is_error_line(line))
        .collect::<Vec<_>>();
    let selected = if error_lines.is_empty() {
        lines.as_slice()
    } else {
        error_lines.as_slice()
    };

    let start = selected.len().saturating_sub(3);
    selected[start..].join(" | ")
}

/// True for stdout noise that should never be shown as the error message.
fn is_ignorable_output_line(line: &str) -> bool {
    let line = line.trim();
    line.eq_ignore_ascii_case("null")
        || line.starts_with('{')
        || line.starts_with('[')
        || line == "NA"
}

/// True for lines that describe an actual yt-dlp failure.
fn is_error_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("ERROR:") || line.starts_with("WARNING:")
}

/// Translates the most common yt-dlp failures into actionable Chinese text.
pub fn friendly_error(message: &str) -> String {
    let lower = message.to_ascii_lowercase();

    if lower.contains("this live event has ended") {
        return "该直播已结束，但 YouTube 尚未生成可下载的回放（通常需要几分钟到数小时）。请稍后重新解析；若已过很久仍失败，可能是该直播没有保留回放。".to_string();
    }

    if lower.contains("requested format is not available") {
        return format!("站点未返回所选视频/音频格式，可能已过期或需要登录 / 地区权限。请更新 Cookie 并重新解析。\n原始错误：{message}");
    }

    if lower.contains("this video is unavailable") || lower.contains("video unavailable") {
        return format!("视频当前不可用：可能已被删除、设为私有、受地区限制，或需要登录后访问。\n原始错误：{message}");
    }

    if lower.contains("sign in to confirm") || lower.contains("login required") {
        return format!("站点要求登录或人机验证，请更新 Cookie 后重试。\n原始错误：{message}");
    }

    if lower.contains("no supported javascript runtime") {
        return "yt-dlp 缺少 JavaScript 运行时，部分 YouTube 格式可能缺失。请安装 Deno 或更新 yt-dlp。".to_string();
    }

    message.to_string()
}
