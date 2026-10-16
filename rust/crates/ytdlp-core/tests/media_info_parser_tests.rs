//! Ported from `YtDlpDownloader.Core.Tests/Services/MediaInfoParserTests.cs`.

use std::time::Duration;
use ytdlp_downloader_core::{JsonMediaInfoParser, MediaInfoParser, ParseError};

fn parser() -> JsonMediaInfoParser {
    JsonMediaInfoParser
}

#[test]
fn parse_full_payload_maps_all_fields() {
    let json = r#"
    {
      "id": "abc123",
      "title": "My Video",
      "webpage_url": "https://youtu.be/abc123",
      "uploader": "Uploader",
      "duration": 65,
      "thumbnail": "https://img/cover.jpg",
      "formats": [
        { "format_id": "137", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "width": 1920, "height": 1080, "fps": 30, "vbr": 4500 },
        { "format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a", "abr": 128 },
        { "format_id": "18", "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a", "width": 640, "height": 360, "fps": 30, "tbr": 800 }
      ]
    }
    "#;

    let info = parser().parse(json).unwrap();

    assert_eq!(info.id, "abc123");
    assert_eq!(info.title, "My Video");
    assert_eq!(info.webpage_url, "https://youtu.be/abc123");
    assert_eq!(info.uploader.as_deref(), Some("Uploader"));
    assert_eq!(info.duration, Some(Duration::from_secs(65)));
    assert_eq!(info.duration_text(), "1:05");
    assert_eq!(info.thumbnail_url.as_deref(), Some("https://img/cover.jpg"));

    assert_eq!(info.videos.len(), 2);
    let combined = info.videos.iter().find(|v| v.format_id == "18").unwrap();
    assert!(combined.has_audio);
    assert_eq!(combined.audio_codec.as_deref(), Some("mp4a"));
    assert_eq!(combined.kind(), "音视频合一");

    assert_eq!(info.audios.len(), 1);
    assert_eq!(info.audios[0].format_id, "140");
    assert_eq!(info.audios[0].bitrate_kbps, Some(128.0));
}

#[test]
fn parse_missing_title_falls_back_to_id() {
    let json = r#"{ "id": "xyz", "formats": [ { "format_id": "1", "vcodec": "avc1", "height": 720 } ] }"#;

    let info = parser().parse(json).unwrap();

    assert_eq!(info.title, "xyz");
    assert_eq!(info.webpage_url, "");
    assert_eq!(info.uploader, None);
    assert_eq!(info.duration, None);
}

#[test]
fn parse_uses_channel_when_uploader_missing() {
    let json = r#"{ "id": "1", "channel": "The Channel", "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }"#;
    assert_eq!(parser().parse(json).unwrap().uploader.as_deref(), Some("The Channel"));
}

#[test]
fn parse_string_numbers_are_parsed() {
    let json = r#"
    {
      "id": "1",
      "duration": "12.5",
      "formats": [ { "format_id": "1", "vcodec": "avc1", "width": "1920", "height": "1080", "fps": "29.97", "vbr": "4500" } ]
    }
    "#;

    let info = parser().parse(json).unwrap();

    assert_eq!(info.duration, Some(Duration::from_secs_f64(12.5)));
    let video = &info.videos[0];
    assert_eq!(video.width, Some(1920));
    assert_eq!(video.height, Some(1080));
    assert_eq!(video.fps, Some(29.97));
    assert_eq!(video.bitrate_kbps, Some(4500.0));
}

#[test]
fn parse_none_and_empty_codecs_are_skipped() {
    let json = r#"
    {
      "id": "1",
      "formats": [
        { "format_id": "1", "vcodec": "none", "acodec": "none" },
        { "format_id": "2", "vcodec": "", "acodec": "" },
        { "format_id": "", "vcodec": "avc1" }
      ]
    }
    "#;

    assert!(matches!(parser().parse(json), Err(ParseError::InvalidData(_))));
}

#[test]
fn parse_no_formats_array_throws() {
    assert!(matches!(
        parser().parse(r#"{ "id": "1" }"#),
        Err(ParseError::InvalidData(_))
    ));
}

#[test]
fn parse_non_object_root_throws() {
    assert!(matches!(parser().parse("[]"), Err(ParseError::InvalidData(_))));
}

#[test]
fn parse_invalid_json_throws_invalid_json() {
    assert!(matches!(parser().parse("not json"), Err(ParseError::InvalidJson(_))));
}

#[test]
fn parse_sorts_videos_by_height_then_bitrate_then_fps() {
    let json = r#"
    {
      "id": "1",
      "formats": [
        { "format_id": "a", "vcodec": "avc1", "height": 1080, "vbr": 3000, "fps": 30 },
        { "format_id": "b", "vcodec": "avc1", "height": 1080, "vbr": 5000, "fps": 30 },
        { "format_id": "c", "vcodec": "avc1", "height": 720,  "vbr": 9000, "fps": 60 },
        { "format_id": "d", "vcodec": "avc1", "height": 1080, "vbr": 5000, "fps": 60 }
      ]
    }
    "#;

    let ids = parser()
        .parse(json)
        .unwrap()
        .videos
        .iter()
        .map(|v| v.format_id.clone())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["d", "b", "a", "c"]);
}

#[test]
fn parse_sorts_audios_by_bitrate_descending() {
    let json = r#"
    {
      "id": "1",
      "formats": [
        { "format_id": "low",  "vcodec": "none", "acodec": "mp4a", "abr": 64 },
        { "format_id": "high", "vcodec": "none", "acodec": "opus", "abr": 256 }
      ]
    }
    "#;

    let ids = parser()
        .parse(json)
        .unwrap()
        .audios
        .iter()
        .map(|a| a.format_id.clone())
        .collect::<Vec<_>>();

    assert_eq!(ids, ["high", "low"]);
}

#[test]
fn parse_thumbnails_array_picks_largest_area() {
    let json = r#"
    {
      "id": "1",
      "formats": [ { "format_id": "1", "vcodec": "avc1" } ],
      "thumbnails": [
        { "url": "small.jpg", "width": 100, "height": 100 },
        { "height": 500 },
        { "url": "large.jpg", "width": 1280, "height": 720 }
      ]
    }
    "#;

    assert_eq!(parser().parse(json).unwrap().thumbnail_url.as_deref(), Some("large.jpg"));
}

#[test]
fn parse_direct_thumbnail_preferred_over_array() {
    let json = r#"
    {
      "id": "1",
      "thumbnail": "direct.jpg",
      "formats": [ { "format_id": "1", "vcodec": "avc1" } ],
      "thumbnails": [ { "url": "array.jpg", "width": 9999, "height": 9999 } ]
    }
    "#;

    assert_eq!(parser().parse(json).unwrap().thumbnail_url.as_deref(), Some("direct.jpg"));
}

#[test]
fn parse_video_only_when_no_audio() {
    let json = r#"{ "id": "1", "formats": [ { "format_id": "1", "vcodec": "avc1", "height": 480 } ] }"#;

    let info = parser().parse(json).unwrap();

    assert!(info.audios.is_empty());
    assert_eq!(info.videos.len(), 1);
    assert_eq!(info.videos[0].kind(), "纯视频");
    assert_eq!(info.videos[0].audio_codec, None);
}

#[test]
fn parse_playlist_playlist_payload_returns_entries() {
    let json = r#"
    {
      "_type": "playlist",
      "id": "PL1",
      "title": "My Playlist",
      "entries": [
        { "_type": "url", "id": "a", "title": "First", "url": "https://example.com/a", "webpage_url": "https://example.com/a" },
        { "_type": "url", "id": "b", "title": "Second", "url": "https://example.com/b" },
        { "_type": "url", "id": "c", "title": "No Url" }
      ]
    }
    "#;

    let playlist = parser().parse_playlist(json).unwrap().unwrap();

    assert_eq!(playlist.id, "PL1");
    assert_eq!(playlist.title, "My Playlist");
    assert_eq!(playlist.entries.len(), 2);
    assert_eq!(playlist.entries[0].url, "https://example.com/a");
    assert_eq!(playlist.entries[0].title, "First");
    assert_eq!(playlist.entries[1].url, "https://example.com/b");
}

#[test]
fn parse_playlist_single_video_returns_none() {
    let json = r#"{ "_type": "video", "id": "1", "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }"#;
    assert_eq!(parser().parse_playlist(json).unwrap(), None);
}

#[test]
fn parse_playlist_empty_entries_returns_empty_playlist() {
    let json = r#"{ "_type": "playlist", "id": "PL1", "title": "Empty", "entries": [] }"#;
    let playlist = parser().parse_playlist(json).unwrap().unwrap();
    assert!(playlist.entries.is_empty());
}

#[test]
fn parse_is_live_flag_sets_is_live() {
    let json = r#"{ "id": "1", "is_live": true, "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }"#;
    let info = parser().parse(json).unwrap();
    assert!(info.is_live);
    assert!(!info.is_upcoming);
}

#[test]
fn parse_live_status_is_live_sets_is_live() {
    let json = r#"{ "id": "1", "live_status": "is_live", "is_live": false, "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }"#;
    assert!(parser().parse(json).unwrap().is_live);
}

#[test]
fn parse_upcoming_stream_sets_is_upcoming_and_allows_missing_formats() {
    let json = r#"{ "id": "1", "title": "Premiere", "live_status": "is_upcoming", "is_live": false }"#;
    let info = parser().parse(json).unwrap();
    assert!(info.is_upcoming);
    assert!(!info.is_live);
    assert!(info.videos.is_empty());
    assert!(info.audios.is_empty());
}

#[test]
fn parse_finished_live_is_treated_as_normal_video() {
    let json = r#"{ "id": "1", "live_status": "was_live", "was_live": true, "is_live": false, "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }"#;
    let info = parser().parse(json).unwrap();
    assert!(!info.is_live);
    assert!(!info.is_upcoming);
}

#[test]
fn parse_null_is_live_is_not_live() {
    let json = r#"{ "id": "1", "is_live": null, "live_status": "not_live", "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }"#;
    assert!(!parser().parse(json).unwrap().is_live);
}