//! Ported from `YtDlpDownloader.Core.Tests/Models/MediaModelTests.cs`.

use std::time::Duration;
use ytdlp_downloader_core::{AudioSource, MediaInfo, VideoSource};

fn video(
    width: Option<i64>,
    height: Option<i64>,
    fps: Option<f64>,
    bitrate: Option<f64>,
    has_audio: bool,
    audio_codec: Option<&str>,
) -> VideoSource {
    VideoSource::new(
        "137",
        "mp4",
        "avc1",
        has_audio,
        audio_codec.map(str::to_string),
        width,
        height,
        fps,
        bitrate,
    )
}

#[test]
fn video_source_resolution_formats_both_dimensions() {
    assert_eq!(video(Some(1920), Some(1080), Some(30.0), Some(4500.0), false, None).resolution(), "1920x1080");
}

#[test]
fn video_source_resolution_missing_dimension_is_dash() {
    assert_eq!(video(Some(1920), None, Some(30.0), Some(4500.0), false, None).resolution(), "-");
}

#[test]
fn video_source_fps_text_null_is_dash() {
    assert_eq!(video(Some(1920), Some(1080), None, None, false, None).fps_text(), "-");
}

#[test]
fn video_source_fps_text_whole_number_has_no_decimal() {
    assert_eq!(video(None, None, Some(29.999), None, false, None).fps_text(), "30");
}

#[test]
fn video_source_fps_text_fractional_keeps_one_decimal() {
    assert_eq!(video(None, None, Some(23.4), None, false, None).fps_text(), "23.4");
}

#[test]
fn video_source_fps_text_fractional_rounds_to_whole_when_close() {
    assert_eq!(video(None, None, Some(23.976), None, false, None).fps_text(), "24");
}

#[test]
fn video_source_bitrate_text_null_is_dash() {
    assert_eq!(video(None, None, None, None, false, None).bitrate_text(), "-");
}

#[test]
fn video_source_bitrate_text_rounds_to_whole_kbps() {
    assert_eq!(video(None, None, None, Some(4499.6), false, None).bitrate_text(), "4500 kbps");
}

#[test]
fn video_source_kind_depends_on_audio_presence() {
    assert_eq!(video(None, None, None, None, false, None).kind(), "纯视频");
    assert_eq!(video(None, None, None, None, true, Some("mp4a")).kind(), "音视频合一");
}

#[test]
fn audio_source_bitrate_text_formats_and_handles_null() {
    assert_eq!(AudioSource::new("140", "m4a", "mp4a", Some(128.4)).bitrate_text(), "128 kbps");
    assert_eq!(AudioSource::new("140", "m4a", "mp4a", None).bitrate_text(), "-");
}

fn info(duration: Option<Duration>) -> MediaInfo {
    MediaInfo::new(
        "id",
        "title",
        "url",
        duration,
        Some("up".to_string()),
        None,
        Vec::new(),
        Vec::new(),
    )
}

#[test]
fn media_info_duration_text_null_is_dash() {
    assert_eq!(info(None).duration_text(), "-");
}

#[test]
fn media_info_duration_text_under_one_hour_is_minutes_seconds() {
    assert_eq!(info(Some(Duration::from_secs(185))).duration_text(), "3:05");
}

#[test]
fn media_info_duration_text_one_hour_or_more_is_hours_minutes_seconds() {
    // 1 hour, 2 minutes, 3 seconds.
    assert_eq!(info(Some(Duration::from_secs(3723))).duration_text(), "1:02:03");
}

#[test]
fn media_info_live_flags_default_to_false() {
    let info = info(None);
    assert!(!info.is_live);
    assert!(!info.is_upcoming);
}

#[test]
fn media_info_live_flags_can_be_set() {
    let mut info = info(None);
    info.is_live = true;
    info.is_upcoming = false;
    assert!(info.is_live);
    assert!(!info.is_upcoming);
}