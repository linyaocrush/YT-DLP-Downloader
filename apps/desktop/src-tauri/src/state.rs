//! Shared application state: the ported core services plus the active-job token.

use crate::error::CommandError;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
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
    job: Mutex<Option<(u64, CancellationToken)>>,
    next_job: AtomicU64,
    pub queue: Mutex<Vec<serde_json::Value>>,
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
            next_job: AtomicU64::new(1),
            queue: Mutex::new(Vec::new()),
        }
    }

    /// Registers a fresh token as the active job and returns it.
    pub fn begin_job(&self) -> Result<JobGuard<'_>, CommandError> {
        let token = CancellationToken::new();
        let mut guard = self
            .job
            .lock()
            .map_err(|_| CommandError::message("任务状态不可用"))?;
        if guard.is_some() {
            return Err(CommandError::message("已有任务运行，请先停止当前任务"));
        }
        let id = self.next_job.fetch_add(1, Ordering::Relaxed);
        *guard = Some((id, token.clone()));
        Ok(JobGuard {
            state: self,
            id,
            token,
        })
    }

    /// Requests cancellation of the active job, if any.
    pub fn cancel_job(&self, id: Option<u64>) {
        if let Ok(guard) = self.job.lock() {
            if let Some((active_id, token)) = guard.as_ref() {
                if id.is_none() || id == Some(*active_id) {
                    token.cancel();
                }
            }
        }
    }

    /// Clears the active job once it has finished.
    pub fn job_status(&self) -> Option<JobStatus> {
        self.job.lock().ok()?.as_ref().map(|(id, token)| JobStatus {
            job_id: *id,
            cancelling: token.is_cancelled(),
        })
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub job_id: u64,
    pub cancelling: bool,
}

pub struct JobGuard<'a> {
    state: &'a AppState,
    pub id: u64,
    pub token: CancellationToken,
}
impl Drop for JobGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.state.job.lock() {
            if guard.as_ref().is_some_and(|(id, _)| *id == self.id) {
                *guard = None;
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_job_cannot_be_overwritten_and_cancel_is_scoped() {
        let state = AppState::new();
        let first = state.begin_job().unwrap();
        assert!(state.begin_job().is_err());
        state.cancel_job(Some(first.id + 1));
        assert!(!first.token.is_cancelled());
        state.cancel_job(Some(first.id));
        assert!(first.token.is_cancelled());
        drop(first);
        let second = state.begin_job().unwrap();
        assert!(!second.token.is_cancelled());
    }
}
