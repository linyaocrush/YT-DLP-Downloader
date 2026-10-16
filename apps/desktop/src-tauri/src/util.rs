//! Small helpers shared by the commands.

use std::path::PathBuf;

use ytdlp_downloader_core::{AppSettings, ProxyRules};

/// Default save folder, mirroring `DownloadViewModel.GetDefaultDownloadDirectory`:
/// `%USERPROFILE%\Downloads`, falling back to the profile directory itself.
pub fn default_download_directory() -> String {
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return std::env::current_dir()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
    };

    let downloads = profile.join("Downloads");
    if downloads.is_dir() {
        downloads.to_string_lossy().into_owned()
    } else {
        profile.to_string_lossy().into_owned()
    }
}

/// Cookie file forwarded to yt-dlp via `--cookies`; `None` when unused.
pub fn cookie_argument(settings: &AppSettings) -> Option<String> {
    let value = settings.cookie_file.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Proxy URL forwarded to yt-dlp via `--proxy` for the given URL; `None` when the
/// URL is not routed through the proxy.
pub fn proxy_argument(settings: &AppSettings, url: &str) -> Option<String> {
    if ProxyRules::should_use_proxy(settings, Some(url)) {
        ProxyRules::build_proxy_url(settings)
    } else {
        None
    }
}

/// Truncates text to `max` characters (adding an ellipsis), UTF-8 safe.
pub fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max || max <= 3 {
        return text.to_string();
    }

    let kept: String = text.chars().take(max - 3).collect();
    format!("{kept}...")
}
