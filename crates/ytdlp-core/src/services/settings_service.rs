//! Settings persistence: JSON file loading/saving plus change notification.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::models::AppSettings;

/// Change listener invoked after a successful save.
pub type SettingsListener = Arc<dyn Fn() + Send + Sync + 'static>;

/// Holds the live [`AppSettings`] and persists them on demand.
pub trait SettingsService: Send + Sync {
    /// Snapshot of the current settings.
    fn settings(&self) -> AppSettings;

    /// Mutates the live settings in place.
    fn update(&self, mutate: &mut dyn FnMut(&mut AppSettings));

    /// Serializes the current settings to disk and notifies listeners.
    /// Never fails the caller: a settings file that cannot be written is ignored.
    fn save(&self);

    /// Registers a listener notified after each successful [`SettingsService::save`].
    fn subscribe(&self, listener: SettingsListener);
}

/// [`SettingsService`] backed by a `settings.json` file.
pub struct FileSettingsService {
    file_path: PathBuf,
    state: Mutex<AppSettings>,
    listeners: Mutex<Vec<SettingsListener>>,
}

impl FileSettingsService {
    /// Creates the service, scanning `directory` (defaulting to
    /// `%APPDATA%\YtDlpDownloader`) and loading an existing `settings.json`.
    /// A missing or corrupted file falls back to defaults.
    pub fn new(directory: Option<PathBuf>) -> Self {
        let directory = directory.unwrap_or_else(default_directory);
        let _ = fs::create_dir_all(&directory);
        let file_path = directory.join("settings.json");
        let state = load(&file_path);

        Self {
            file_path,
            state: Mutex::new(state),
            listeners: Mutex::new(Vec::new()),
        }
    }

    /// Path of the JSON file this service reads and writes.
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
}

impl SettingsService for FileSettingsService {
    fn settings(&self) -> AppSettings {
        self.state
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn update(&self, mutate: &mut dyn FnMut(&mut AppSettings)) {
        if let Ok(mut guard) = self.state.lock() {
            mutate(&mut guard);
        }
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.settings()) {
            let _ = fs::write(&self.file_path, json);
        }
        notify(&self.listeners);
    }

    fn subscribe(&self, listener: SettingsListener) {
        if let Ok(mut guard) = self.listeners.lock() {
            guard.push(listener);
        }
    }
}

fn notify(listeners: &Mutex<Vec<SettingsListener>>) {
    // Clone the list first so a listener that calls `save()` cannot deadlock.
    let snapshot = match listeners.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return,
    };

    for listener in snapshot {
        listener();
    }
}

fn load(file_path: &Path) -> AppSettings {
    fs::read_to_string(file_path)
        .ok()
        .and_then(|text| serde_json::from_str::<AppSettings>(&text).ok())
        .unwrap_or_default()
}

fn default_directory() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("YtDlpDownloader")
}
