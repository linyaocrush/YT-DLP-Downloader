//! Shared test helpers. Compiled into every integration test binary, so unused
//! items are expected in any single binary.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use ytdlp_downloader_core::{AppSettings, SettingsListener, SettingsService};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Self-cleaning temporary directory.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> Self {
        let unique = format!(
            "ytdlp-rs-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Creates an empty file (and any parent directories), returning its path.
    pub fn create_file(&self, relative: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(&path, b"").expect("write file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// In-memory [`SettingsService`] used by the path-service tests, counting saves.
pub struct FakeSettings {
    state: Mutex<AppSettings>,
    saves: AtomicUsize,
    listeners: Mutex<Vec<SettingsListener>>,
}

impl FakeSettings {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            state: Mutex::new(settings),
            saves: AtomicUsize::new(0),
            listeners: Mutex::new(Vec::new()),
        }
    }

    pub fn save_count(&self) -> usize {
        self.saves.load(Ordering::SeqCst)
    }

    pub fn snapshot(&self) -> AppSettings {
        self.state.lock().unwrap().clone()
    }
}

impl SettingsService for FakeSettings {
    fn settings(&self) -> AppSettings {
        self.state.lock().unwrap().clone()
    }

    fn update(&self, mutate: &mut dyn FnMut(&mut AppSettings)) {
        let mut guard = self.state.lock().unwrap();
        mutate(&mut guard);
    }

    fn save(&self) {
        self.saves.fetch_add(1, Ordering::SeqCst);
        let listeners = self.listeners.lock().unwrap().clone();
        for listener in listeners {
            listener();
        }
    }

    fn subscribe(&self, listener: SettingsListener) {
        self.listeners.lock().unwrap().push(listener);
    }
}
