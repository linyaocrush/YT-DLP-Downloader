//! Settings persistence: JSON file loading/saving plus change notification.

use std::fs;
use std::io::{self, Write};
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

    fn try_save(&self) -> io::Result<()> {
        self.save();
        Ok(())
    }

    fn load_warning(&self) -> Option<String> {
        None
    }

    /// Registers a listener notified after each successful [`SettingsService::save`].
    fn subscribe(&self, listener: SettingsListener);
}

/// [`SettingsService`] backed by a `settings.json` file.
pub struct FileSettingsService {
    file_path: PathBuf,
    state: Mutex<AppSettings>,
    listeners: Mutex<Vec<SettingsListener>>,
    warning: Option<String>,
}

impl FileSettingsService {
    /// Creates the service, scanning `directory` (defaulting to
    /// `%APPDATA%\YtDlpDownloader`) and loading an existing `settings.json`.
    /// A missing or corrupted file falls back to defaults.
    pub fn new(directory: Option<PathBuf>) -> Self {
        let directory = directory.unwrap_or_else(default_directory);
        let _ = fs::create_dir_all(&directory);
        let file_path = directory.join("settings.json");
        let (state, warning) = load(&file_path);

        Self {
            file_path,
            state: Mutex::new(state),
            listeners: Mutex::new(Vec::new()),
            warning,
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
        let _ = self.try_save();
    }

    fn try_save(&self) -> io::Result<()> {
        if self
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("备份失败") || warning.contains("无法读取配置"))
        {
            return Err(io::Error::other("损坏配置备份失败，已阻止覆盖原文件"));
        }
        let json = serde_json::to_vec_pretty(&self.settings())?;
        atomic_write(&self.file_path, &json)?;
        notify(&self.listeners);
        Ok(())
    }

    fn load_warning(&self) -> Option<String> {
        self.warning.clone()
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

pub fn atomic_write(file_path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = file_path
        .parent()
        .ok_or_else(|| io::Error::other("缺少父目录"))?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(file_path).map_err(|error| error.error)?;
    Ok(())
}

fn load(file_path: &Path) -> (AppSettings, Option<String>) {
    match fs::read(file_path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(settings) => (settings, None),
            Err(error) => {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let backup = file_path.with_extension(format!("corrupt-{stamp}.json"));
                let message = match fs::write(&backup, bytes) {
                    Ok(()) => format!(
                        "配置损坏，已加载默认值。原配置备份：{}（{error}）",
                        backup.display()
                    ),
                    Err(backup_error) => {
                        format!("配置损坏且备份失败，原文件将保留：{backup_error}")
                    }
                };
                (AppSettings::default(), Some(message))
            }
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => (AppSettings::default(), None),
        Err(error) => (
            AppSettings::default(),
            Some(format!("无法读取配置：{error}")),
        ),
    }
}

fn default_directory() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("YtDlpDownloader")
}
