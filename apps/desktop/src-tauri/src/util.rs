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
pub fn validate_cookie(path: &str) -> Result<(), String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("无法读取 Cookie 文件：{error}"))?;
    if !text
        .lines()
        .take(3)
        .any(|line| line.contains("HTTP Cookie File"))
    {
        return Err("Cookie 文件需要 Netscape HTTP Cookie File 格式，请重新导出".into());
    }
    let mut found = false;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if line.starts_with('#') && !line.starts_with("#HttpOnly_") {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 7
            || fields[0].is_empty()
            || fields[5].is_empty()
            || fields[4].parse::<u64>().is_err()
        {
            return Err("Cookie 数据行无效，请重新导出 Netscape 格式".into());
        }
        found = true;
    }
    if !found {
        return Err("Cookie 文件不包含有效记录".into());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cookie_validation_rejects_unrelated_and_missing_files() {
        let directory =
            std::env::temp_dir().join(format!("ytdlp-cookie-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("cookies.txt");
        std::fs::write(
            &path,
            "# Netscape HTTP Cookie File\n.example.com\tTRUE\t/\tTRUE\t0\tsession\tvalue\n",
        )
        .unwrap();
        assert!(validate_cookie(&path.to_string_lossy()).is_ok());
        std::fs::write(&path, "unrelated content").unwrap();
        assert!(validate_cookie(&path.to_string_lossy()).is_err());
        std::fs::remove_file(&path).unwrap();
        assert!(validate_cookie(&path.to_string_lossy()).is_err());
        std::fs::remove_dir(directory).unwrap();
    }
}
