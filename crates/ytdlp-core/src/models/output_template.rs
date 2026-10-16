//! yt-dlp output-template fields and the naming rules around them.
//!
//! Ports the field-chip logic that lived in the WPF `DownloadViewModel`
//! (catalog, token selection and the two name modes) into UI-independent code.

use super::app_settings::OutputNameMode;

/// Token suffix appending the media extension; never part of the stored fields.
pub const EXTENSION_TOKEN: &str = "%(ext)s";

/// yt-dlp output-template fields offered as chips, with their display labels.
pub const TEMPLATE_FIELD_CATALOG: &[(&str, &str)] = &[
    ("%(title)s", "标题"),
    ("%(id)s", "视频 ID"),
    ("%(uploader)s", "上传者"),
    ("%(upload_date)s", "上传日期"),
    ("%(resolution)s", "分辨率"),
    ("%(fps)s", "帧率"),
    ("%(duration)s", "时长"),
    ("%(extractor)s", "站点"),
    ("%(format_id)s", "格式 ID"),
    ("%(vcodec)s", "视频编码"),
    ("%(acodec)s", "音频编码"),
];

/// True when `template` already contains `token` (case-insensitive).
pub fn is_token_selected(template: &str, token: &str) -> bool {
    !template.is_empty() && template.to_lowercase().contains(&token.to_lowercase())
}

/// Joins the selected tokens with spaces; this is the value stored in settings.
pub fn join_fields<'a, I>(tokens: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    tokens.into_iter().collect::<Vec<_>>().join(" ")
}

/// The full template (fields + extension) handed to yt-dlp as `-o`.
pub fn full_template(fields: &str) -> String {
    format!("{fields}.{EXTENSION_TOKEN}")
}

/// Resolves the `-o` template passed to yt-dlp for the configured naming mode.
///
/// Returns `None` when nothing usable is configured so the caller can fall back
/// to yt-dlp's own default.
pub fn build_output_template(
    mode: OutputNameMode,
    fixed_file_name: &str,
    template: &str,
) -> Option<String> {
    match mode {
        OutputNameMode::Fixed => {
            let name = fixed_file_name.trim();
            if name.is_empty() {
                return None;
            }

            if name.to_lowercase().contains(EXTENSION_TOKEN) {
                Some(name.to_string())
            } else {
                Some(format!("{name}.{EXTENSION_TOKEN}"))
            }
        }
        OutputNameMode::Template => {
            if template.trim().is_empty() {
                None
            } else {
                Some(template.to_string())
            }
        }
    }
}
