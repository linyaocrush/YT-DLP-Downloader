//! UI-independent domain logic for the YT-DLP Downloader.
//!
//! This crate is a Rust port of the C# `YtDlpDownloader.Core` assembly. It holds
//! the models, yt-dlp argument / `--dump-single-json` handling, settings
//! persistence, proxy rules and the process runner, with no UI dependencies
//! (the process runner does use Windows APIs to kill process trees, but only
//! behind `#[cfg(windows)]`).
//!
//! The WPF-era MVVM base classes (`ObservableObject`, `RelayCommand`,
//! `AsyncRelayCommand`) intentionally have no equivalent here: the Tauri
//! frontend owns UI state, so those types are dropped rather than translated.

pub mod cancellation;
pub mod models;
pub mod services;

pub use cancellation::CancellationToken;
pub use models::{
    output_template, AppSettings, AppTheme, AudioSource, DownloadKind, MediaInfo, OutputNameMode,
    PlaylistEntry, PlaylistInfo, ProxyListMode, ProxyProtocol, ProxyRules, ResolutionPreference,
    VideoSource, YtDlpFormat,
};
pub use services::{
    ffmpeg_path_service, last_meaningful, locate_in_path, media_info_parser, process_runner,
    settings_service, ytdlp_cli, ytdlp_path_service, DownloadOutcome, DownloadRequest,
    DownloadUpdate, FfmpegPathService, FileSettingsService, JsonMediaInfoParser, LineCallback,
    MediaInfoParser, MediaParseResult, ParseError, PathEnvProvider, PathFfmpegService,
    PathYtDlpService, ProcessError, ProcessResult, ProcessRunner, ProgressCallback,
    SettingsListener, SettingsService, TokioProcessRunner, YtDlpCli, YtDlpError, YtDlpPathService,
    NOT_CONFIGURED_MESSAGE,
};
