//! Ported from `YtDlpDownloader.Core.Tests/Services/SettingsServiceTests.cs`.

mod common;

use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::TempDir;
use ytdlp_downloader_core::{
    AppTheme, DownloadKind, FileSettingsService, OutputNameMode, ProxyListMode,
    ResolutionPreference, SettingsService,
};

#[test]
fn constructor_no_file_uses_defaults() {
    let temp = TempDir::new();
    let service = FileSettingsService::new(Some(temp.path().to_path_buf()));

    let settings = service.settings();
    assert_eq!(settings.download_threads, 4);
    assert_eq!(settings.output_template, "%(title)s");
    assert_eq!(settings.theme, AppTheme::System);
    assert_eq!(settings.output_name_mode, OutputNameMode::Template);
    assert_eq!(settings.yt_dlp_path, "");
}

#[test]
fn save_then_reload_round_trips_persisted_values() {
    let temp = TempDir::new();

    let first = FileSettingsService::new(Some(temp.path().to_path_buf()));
    first.update(&mut |settings| {
        settings.yt_dlp_path = "C:/yt-dlp.exe".to_string();
        settings.download_directory = "C:/downloads".to_string();
        settings.theme = AppTheme::Dark;
        settings.download_kind = DownloadKind::AudioOnly;
        settings.resolution = ResolutionPreference::UpTo1080P;
        settings.proxy_enabled = true;
        settings.proxy_list_mode = ProxyListMode::Blacklist;
        settings.proxy_sites.push("youtube.com".to_string());
    });
    first.save();

    let reloaded = FileSettingsService::new(Some(temp.path().to_path_buf()));
    let settings = reloaded.settings();

    assert_eq!(settings.yt_dlp_path, "C:/yt-dlp.exe");
    assert_eq!(settings.download_directory, "C:/downloads");
    assert_eq!(settings.theme, AppTheme::Dark);
    assert_eq!(settings.download_kind, DownloadKind::AudioOnly);
    assert_eq!(settings.resolution, ResolutionPreference::UpTo1080P);
    assert!(settings.proxy_enabled);
    assert_eq!(settings.proxy_list_mode, ProxyListMode::Blacklist);
    assert_eq!(settings.proxy_sites, ["youtube.com"]);
}

#[test]
fn save_writes_enums_as_strings() {
    let temp = TempDir::new();
    let service = FileSettingsService::new(Some(temp.path().to_path_buf()));
    service.update(&mut |settings| {
        settings.theme = AppTheme::Dark;
        settings.download_kind = DownloadKind::AudioOnly;
    });

    service.save();

    let text = fs::read_to_string(service.file_path()).unwrap();
    assert!(text.contains("\"Dark\""), "{text}");
    assert!(text.contains("\"AudioOnly\""), "{text}");
}

#[test]
fn constructor_corrupted_file_falls_back_to_defaults() {
    let temp = TempDir::new();
    fs::write(
        temp.path().join("settings.json"),
        "{ this is not valid json",
    )
    .unwrap();

    let service = FileSettingsService::new(Some(temp.path().to_path_buf()));
    let settings = service.settings();

    assert_eq!(settings.download_threads, 4);
    assert_eq!(settings.theme, AppTheme::System);
}

#[test]
fn save_raises_changed_event() {
    let temp = TempDir::new();
    let service = FileSettingsService::new(Some(temp.path().to_path_buf()));
    let raised = Arc::new(AtomicUsize::new(0));

    let sink = Arc::clone(&raised);
    service.subscribe(Arc::new(move || {
        sink.fetch_add(1, Ordering::SeqCst);
    }));

    service.save();
    service.save();

    assert_eq!(raised.load(Ordering::SeqCst), 2);
}
