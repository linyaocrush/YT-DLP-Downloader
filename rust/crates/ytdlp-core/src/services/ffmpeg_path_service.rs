//! Resolves the ffmpeg executable handed to yt-dlp via `--ffmpeg-location`.

use std::path::Path;
use std::sync::Arc;

use super::settings_service::SettingsService;
use super::ytdlp_path_service::{scan_for_executable, PathEnvProvider};

/// Executable name looked up on `PATH`.
pub const FFMPEG_EXE: &str = "ffmpeg.exe";

pub trait FfmpegPathService: Send + Sync {
    /// Scans every directory of `PATH` for ffmpeg.
    fn locate_in_path(&self) -> Option<String>;

    /// Returns the manually configured path when it still exists, otherwise `None`.
    fn configured_path(&self) -> Option<String>;

    /// Path to hand to yt-dlp via `--ffmpeg-location`, or `None` when nothing
    /// should be passed. ffmpeg on `PATH` is resolved by yt-dlp itself, so only a
    /// manual (non-`PATH`) path is returned.
    fn explicit_path_for_yt_dlp(&self) -> Option<String>;
}

/// Default [`FfmpegPathService`] backed by settings + the process `PATH`.
pub struct PathFfmpegService {
    settings: Arc<dyn SettingsService>,
    path_env: PathEnvProvider,
}

impl PathFfmpegService {
    pub fn new(settings: Arc<dyn SettingsService>) -> Self {
        Self::with_path_env(settings, Arc::new(|| std::env::var("PATH").ok()))
    }

    pub fn with_path_env(settings: Arc<dyn SettingsService>, path_env: PathEnvProvider) -> Self {
        Self { settings, path_env }
    }
}

impl FfmpegPathService for PathFfmpegService {
    fn locate_in_path(&self) -> Option<String> {
        let value = (self.path_env)()?;
        scan_for_executable(&value, FFMPEG_EXE)
    }

    fn configured_path(&self) -> Option<String> {
        let configured = self.settings.settings().ffmpeg_path.trim().to_string();
        if !configured.is_empty() && Path::new(&configured).is_file() {
            Some(configured)
        } else {
            None
        }
    }

    fn explicit_path_for_yt_dlp(&self) -> Option<String> {
        if self.locate_in_path().is_some() {
            None
        } else {
            self.configured_path()
        }
    }
}