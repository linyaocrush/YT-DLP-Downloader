//! Ported from `YtDlpDownloader.Core.Tests/Services/PathServiceTests.cs`.
//!
//! The C# tests mutate the process-wide `PATH` environment variable. That is not
//! thread-safe (and `std::env::set_var` is unsafe in edition 2024), so the port
//! injects the `PATH` value through [`PathEnvProvider`] instead.

mod common;

use std::sync::Arc;

use common::{FakeSettings, TempDir};
use ytdlp_downloader_core::{
    AppSettings, FfmpegPathService, PathEnvProvider, PathFfmpegService, PathYtDlpService,
    SettingsService, YtDlpPathService,
};

fn path_env(value: Option<String>) -> PathEnvProvider {
    Arc::new(move || value.clone())
}

fn dyn_settings(settings: Arc<FakeSettings>) -> Arc<dyn SettingsService> {
    settings
}

#[test]
fn ffmpeg_get_configured_path_existing_file_returns_path() {
    let temp = TempDir::new();
    let executable = temp.create_file("ffmpeg.exe");
    let settings = Arc::new(FakeSettings::new(AppSettings {
        ffmpeg_path: executable.to_string_lossy().into_owned(),
        ..Default::default()
    }));
    let service = PathFfmpegService::new(dyn_settings(settings));

    assert_eq!(
        service.configured_path(),
        Some(executable.to_string_lossy().into_owned())
    );
}

#[test]
fn ffmpeg_get_configured_path_missing_file_returns_null() {
    let temp = TempDir::new();
    let settings = Arc::new(FakeSettings::new(AppSettings {
        ffmpeg_path: temp.path().join("nope.exe").to_string_lossy().into_owned(),
        ..Default::default()
    }));
    let service = PathFfmpegService::new(dyn_settings(settings));

    assert_eq!(service.configured_path(), None);
}

#[test]
fn ffmpeg_get_configured_path_blank_returns_null() {
    for configured in ["", "   "] {
        let settings = Arc::new(FakeSettings::new(AppSettings {
            ffmpeg_path: configured.to_string(),
            ..Default::default()
        }));
        let service = PathFfmpegService::new(dyn_settings(settings));
        assert_eq!(service.configured_path(), None);
    }
}

#[test]
fn ffmpeg_explicit_path_for_yt_dlp_no_path_hit_returns_configured_path() {
    let temp = TempDir::new();
    let empty = temp.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let ffmpeg = temp.create_file("manual/ffmpeg.exe");

    let settings = Arc::new(FakeSettings::new(AppSettings {
        ffmpeg_path: ffmpeg.to_string_lossy().into_owned(),
        ..Default::default()
    }));
    let service = PathFfmpegService::with_path_env(
        dyn_settings(settings),
        path_env(Some(empty.to_string_lossy().into_owned())),
    );

    assert_eq!(
        service.explicit_path_for_yt_dlp(),
        Some(ffmpeg.to_string_lossy().into_owned())
    );
}

#[test]
fn ffmpeg_locate_in_path_no_path_variable_returns_null() {
    let settings = Arc::new(FakeSettings::new(AppSettings::default()));
    let service = PathFfmpegService::with_path_env(dyn_settings(settings), path_env(None));

    assert_eq!(service.locate_in_path(), None);
}

#[test]
fn ytdlp_resolve_configured_existing_returns_without_save() {
    let temp = TempDir::new();
    let executable = temp.create_file("yt-dlp.exe");
    let settings = Arc::new(FakeSettings::new(AppSettings {
        yt_dlp_path: executable.to_string_lossy().into_owned(),
        ..Default::default()
    }));
    let service = PathYtDlpService::new(dyn_settings(Arc::clone(&settings)));

    assert_eq!(service.resolve(), Some(executable.to_string_lossy().into_owned()));
    assert_eq!(settings.save_count(), 0);
}

#[test]
fn ytdlp_resolve_found_on_path_persists_and_returns() {
    let temp = TempDir::new();
    let executable = temp.create_file("yt-dlp.exe");
    let settings = Arc::new(FakeSettings::new(AppSettings::default()));
    let service = PathYtDlpService::with_path_env(
        dyn_settings(Arc::clone(&settings)),
        path_env(Some(temp.path().to_string_lossy().into_owned())),
    );

    let result = service.resolve();

    assert_eq!(result, Some(executable.to_string_lossy().into_owned()));
    assert_eq!(
        settings.snapshot().yt_dlp_path,
        executable.to_string_lossy().into_owned()
    );
    assert_eq!(settings.save_count(), 1);
}

#[test]
fn ytdlp_resolve_nothing_configured_or_on_path_returns_null() {
    let temp = TempDir::new();
    let empty = temp.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();

    let settings = Arc::new(FakeSettings::new(AppSettings {
        yt_dlp_path: temp.path().join("missing.exe").to_string_lossy().into_owned(),
        ..Default::default()
    }));
    let service = PathYtDlpService::with_path_env(
        dyn_settings(Arc::clone(&settings)),
        path_env(Some(empty.to_string_lossy().into_owned())),
    );

    assert_eq!(service.resolve(), None);
    assert_eq!(settings.save_count(), 0);
}