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
    first.save().unwrap();

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

    service.save().unwrap();

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

    service.save().unwrap();
    service.save().unwrap();

    assert_eq!(raised.load(Ordering::SeqCst), 2);
}

#[test]
fn failed_atomic_save_preserves_original_and_does_not_notify() {
    let temp = TempDir::new();
    let service = FileSettingsService::new(Some(temp.path().to_path_buf()));
    service.save().unwrap();
    let original = fs::read(service.file_path()).unwrap();
    let mut permissions = fs::metadata(service.file_path()).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(service.file_path(), permissions.clone()).unwrap();
    let raised = Arc::new(AtomicUsize::new(0));
    let sink = raised.clone();
    service.subscribe(Arc::new(move || {
        sink.fetch_add(1, Ordering::SeqCst);
    }));
    service.update(&mut |settings| settings.download_threads = 12);
    assert!(service.save().is_err());
    assert_eq!(fs::read(service.file_path()).unwrap(), original);
    assert_eq!(raised.load(Ordering::SeqCst), 0);
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    fs::set_permissions(service.file_path(), permissions).unwrap();
}

#[test]
fn corrupted_configuration_has_recoverable_backup() {
    let temp = TempDir::new();
    let broken = b"{ invalid config";
    fs::write(temp.path().join("settings.json"), broken).unwrap();
    let service = FileSettingsService::new(Some(temp.path().to_path_buf()));
    assert!(service.load_warning().unwrap().contains("备份"));
    service.save().unwrap();
    let backup = fs::read_dir(temp.path())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().contains("corrupt-"))
        .unwrap();
    assert_eq!(fs::read(backup.path()).unwrap(), broken);
}
