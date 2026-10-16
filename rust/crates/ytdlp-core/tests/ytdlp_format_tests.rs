//! Ported from `YtDlpDownloader.Core.Tests/Models/YtDlpFormatTests.cs`.

use ytdlp_downloader_core::{DownloadKind, ResolutionPreference, YtDlpFormat};

#[test]
fn build_expression_default_best_uses_auto_expression() {
    assert_eq!(
        YtDlpFormat::build_expression(DownloadKind::Default, ResolutionPreference::Best),
        "bv*+ba/b"
    );
}

#[test]
fn build_expression_video_only_best_is_bv() {
    assert_eq!(
        YtDlpFormat::build_expression(DownloadKind::VideoOnly, ResolutionPreference::Best),
        "bv"
    );
}

#[test]
fn build_expression_audio_only_ignores_resolution() {
    assert_eq!(
        YtDlpFormat::build_expression(DownloadKind::AudioOnly, ResolutionPreference::UpTo1080P),
        "ba"
    );
}

#[test]
fn build_expression_video_only_applies_resolution_filter() {
    let cases = [
        (ResolutionPreference::Above4K, "bv[height>2160]"),
        (ResolutionPreference::UpTo4K, "bv[height<=2160]"),
        (ResolutionPreference::UpTo2K, "bv[height<=1440]"),
        (ResolutionPreference::UpTo1080P, "bv[height<=1080]"),
        (ResolutionPreference::UpTo720P, "bv[height<=720]"),
        (ResolutionPreference::UpTo360P, "bv[height<=360]"),
    ];

    for (preference, expected) in cases {
        assert_eq!(
            YtDlpFormat::build_expression(DownloadKind::VideoOnly, preference),
            expected
        );
    }
}

#[test]
fn build_expression_default_with_filter_applies_filter_to_both_streams() {
    assert_eq!(
        YtDlpFormat::build_expression(DownloadKind::Default, ResolutionPreference::UpTo720P),
        "bv*[height<=720]+ba/b[height<=720]"
    );
}