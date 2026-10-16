//! Ported from the WPF `OutputTemplateFieldTests` and the naming logic in
//! `DownloadViewModel` (`IsTokenSelected`, `RebuildOutputTemplate`,
//! `BuildOutputTemplate`).

use ytdlp_downloader_core::models::output_template::{
    build_output_template, full_template, is_token_selected, join_fields, TEMPLATE_FIELD_CATALOG,
};
use ytdlp_downloader_core::OutputNameMode;

#[test]
fn catalog_contains_title_token_and_label() {
    assert!(TEMPLATE_FIELD_CATALOG
        .iter()
        .any(|(token, label)| *token == "%(title)s" && *label == "标题"));
    // No duplicate tokens.
    let mut tokens = TEMPLATE_FIELD_CATALOG
        .iter()
        .map(|(t, _)| *t)
        .collect::<Vec<_>>();
    tokens.sort_unstable();
    let count = tokens.len();
    tokens.dedup();
    assert_eq!(tokens.len(), count);
}

#[test]
fn is_token_selected_is_case_insensitive_and_blank_safe() {
    assert!(is_token_selected("%(title)s %(id)s", "%(title)s"));
    assert!(is_token_selected("%(Title)S", "%(title)s"));
    assert!(!is_token_selected("%(title)s", "%(id)s"));
    assert!(!is_token_selected("", "%(title)s"));
}

#[test]
fn join_and_full_template() {
    assert_eq!(join_fields(["%(title)s", "%(id)s"]), "%(title)s %(id)s");
    assert_eq!(full_template("%(title)s"), "%(title)s.%(ext)s");
}

#[test]
fn build_output_template_fixed_mode_appends_extension() {
    assert_eq!(
        build_output_template(OutputNameMode::Fixed, "my-video", "").as_deref(),
        Some("my-video.%(ext)s")
    );
    // An explicit %(ext)s is respected.
    assert_eq!(
        build_output_template(OutputNameMode::Fixed, "my-video.%(ext)s", "").as_deref(),
        Some("my-video.%(ext)s")
    );
    // Blank fixed names fall back to yt-dlp's default.
    assert_eq!(
        build_output_template(OutputNameMode::Fixed, "   ", ""),
        None
    );
}

#[test]
fn build_output_template_template_mode_returns_template_or_none() {
    assert_eq!(
        build_output_template(OutputNameMode::Template, "", "%(title)s.%(ext)s").as_deref(),
        Some("%(title)s.%(ext)s")
    );
    assert_eq!(
        build_output_template(OutputNameMode::Template, "", "   "),
        None
    );
}
