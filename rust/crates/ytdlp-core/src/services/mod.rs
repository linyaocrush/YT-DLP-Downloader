//! Domain services ported from `YtDlpDownloader.Core.Services`.

pub mod ffmpeg_path_service;
pub mod media_info_parser;
pub mod process_runner;
pub mod settings_service;
pub mod ytdlp_cli;
pub mod ytdlp_path_service;

pub use ffmpeg_path_service::{FfmpegPathService, PathFfmpegService};
pub use media_info_parser::{JsonMediaInfoParser, MediaInfoParser, ParseError};
pub use process_runner::{
    LineCallback, ProcessError, ProcessResult, ProcessRunner, TokioProcessRunner,
};
pub use settings_service::{FileSettingsService, SettingsListener, SettingsService};
pub use ytdlp_cli::{
    last_meaningful, DownloadOutcome, DownloadRequest, DownloadUpdate, MediaParseResult,
    ProgressCallback, YtDlpCli, YtDlpError, NOT_CONFIGURED_MESSAGE,
};
pub use ytdlp_path_service::{locate_in_path, PathEnvProvider, PathYtDlpService, YtDlpPathService};
