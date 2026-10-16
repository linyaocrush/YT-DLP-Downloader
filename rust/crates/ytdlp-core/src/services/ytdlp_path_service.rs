//! Resolves the effective `yt-dlp.exe` path (configured, else on `PATH`).

use std::path::Path;
use std::sync::Arc;

use super::settings_service::SettingsService;

/// Executable name looked up on `PATH`.
pub const YT_DLP_EXE: &str = "yt-dlp.exe";

/// Supplies the value of the `PATH` environment variable. Injectable so tests do
/// not have to mutate process-wide environment state.
pub type PathEnvProvider = Arc<dyn Fn() -> Option<String> + Send + Sync + 'static>;

/// Scans every directory of the `PATH` environment variable for `yt-dlp`.
pub fn locate_in_path(file_name: &str) -> Option<String> {
    scan_for_executable(&std::env::var("PATH").ok()?, file_name)
}

/// Scans a raw `PATH`-style string (already split on the platform separator)
/// for a file with the given name.
pub fn scan_for_executable(path_value: &str, file_name: &str) -> Option<String> {
    if path_value.trim().is_empty() {
        return None;
    }

    for directory in std::env::split_paths(path_value) {
        if directory.as_os_str().is_empty() {
            continue;
        }

        let candidate = directory.join(file_name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }

    None
}

pub trait YtDlpPathService: Send + Sync {
    fn locate_in_path(&self) -> Option<String>;

    /// Effective executable: the configured path when it still exists, otherwise
    /// a `PATH` search (persisting a hit into settings).
    fn resolve(&self) -> Option<String>;
}

/// Default [`YtDlpPathService`] backed by settings + the process `PATH`.
pub struct PathYtDlpService {
    settings: Arc<dyn SettingsService>,
    path_env: PathEnvProvider,
}

impl PathYtDlpService {
    pub fn new(settings: Arc<dyn SettingsService>) -> Self {
        Self::with_path_env(settings, Arc::new(|| std::env::var("PATH").ok()))
    }

    pub fn with_path_env(settings: Arc<dyn SettingsService>, path_env: PathEnvProvider) -> Self {
        Self { settings, path_env }
    }
}

impl YtDlpPathService for PathYtDlpService {
    fn locate_in_path(&self) -> Option<String> {
        let value = (self.path_env)()?;
        scan_for_executable(&value, YT_DLP_EXE)
    }

    fn resolve(&self) -> Option<String> {
        let configured = self.settings.settings().yt_dlp_path.trim().to_string();
        if !configured.is_empty() && Path::new(&configured).is_file() {
            return Some(configured);
        }

        let found = self.locate_in_path()?;
        self.settings
            .update(&mut |settings| settings.yt_dlp_path = found.clone());
        self.settings.save();
        Some(found)
    }
}
