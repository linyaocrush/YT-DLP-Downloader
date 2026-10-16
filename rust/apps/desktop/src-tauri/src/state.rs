//! Shared application state: the ported core services plus the active-job token.

use std::sync::{Arc, Mutex};

use ytdlp_downloader_core::{
    CancellationToken, FfmpegPathService, FileSettingsService, JsonMediaInfoParser,
    MediaInfoParser, PathFfmpegService, PathYtDlpService, ProcessRunner, SettingsService,
    TokioProcessRunner, YtDlpCli, YtDlpPathService,
};

/// Process-wide services, created once at start-up (the Rust equivalent of the
/// C# composition root in `App.xaml.cs`).
pub struct AppState {
    pub settings: Arc<dyn SettingsService>,
    pub paths: Arc<dyn YtDlpPathService>,
    pub ffmpeg_paths: Arc<dyn FfmpegPathService>,
    pub cli: Arc<YtDlpCli>,
    /// Cancellation token of the parse-or-download currently in flight. Only one
    /// operation runs at a time, matching the single `_cts` in the WPF view-model.
    job: Mutex<Option<CancellationToken>>,
}

impl AppState {
    pub fn new() -> Self {
        // Trait objects are coerced through an intermediate binding: passing
        // `Arc::new(Concrete)` straight into a `dyn` parameter makes `Arc::new`
        // infer its type argument from the parameter instead.
        let settings: Arc<dyn SettingsService> = Arc::new(FileSettingsService::new(None));

        let paths_concrete = Arc::new(PathYtDlpService::new(Arc::clone(&settings)));
        let paths: Arc<dyn YtDlpPathService> = paths_concrete;

        let ffmpeg_concrete = Arc::new(PathFfmpegService::new(Arc::clone(&settings)));
        let ffmpeg_paths: Arc<dyn FfmpegPathService> = ffmpeg_concrete;

        let runner_concrete = Arc::new(TokioProcessRunner);
        let runner: Arc<dyn ProcessRunner> = runner_concrete;

        let parser_concrete = Arc::new(JsonMediaInfoParser);
        let parser: Arc<dyn MediaInfoParser> = parser_concrete;

        let cli = Arc::new(YtDlpCli::new(
            runner,
            Arc::clone(&paths),
            Arc::clone(&ffmpeg_paths),
            parser,
        ));

        Self {
            settings,
            paths,
            ffmpeg_paths,
            cli,
            job: Mutex::new(None),
        }
    }

    /// Registers a fresh token as the active job and returns it.
    pub fn begin_job(&self) -> CancellationToken {
        let token = CancellationToken::new();
        if let Ok(mut guard) = self.job.lock() {
            *guard = Some(token.clone());
        }
        token
    }

    /// Requests cancellation of the active job, if any.
    pub fn cancel_job(&self) {
        if let Ok(guard) = self.job.lock() {
            if let Some(token) = guard.as_ref() {
                token.cancel();
            }
        }
    }

    /// Clears the active job once it has finished.
    pub fn end_job(&self) {
        if let Ok(mut guard) = self.job.lock() {
            *guard = None;
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
