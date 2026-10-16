//! Maps `yt-dlp --dump-single-json` output onto [`MediaInfo`] / [`PlaylistInfo`].

use std::time::Duration;

use serde_json::Value;

use crate::models::{AudioSource, MediaInfo, PlaylistEntry, PlaylistInfo, VideoSource};

/// Parse failure. Mirrors the split the C# code relied on:
/// * `InvalidData` corresponds to `InvalidDataException` and is recoverable
///   (a flat single-video listing without formats triggers a re-query);
/// * `InvalidJson` corresponds to `System.Text.Json.JsonException` and is fatal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidData(String),
    InvalidJson(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::InvalidData(message) | ParseError::InvalidJson(message) => {
                write!(f, "{message}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parses yt-dlp JSON output into the domain models.
pub trait MediaInfoParser: Send + Sync {
    fn parse(&self, json: &str) -> Result<MediaInfo, ParseError>;

    /// Returns `Some` when `json` describes a playlist, otherwise `None` so the
    /// caller can fall back to a single-video parse.
    fn parse_playlist(&self, json: &str) -> Result<Option<PlaylistInfo>, ParseError>;
}

/// Default [`MediaInfoParser`] backed by `serde_json`.
///
/// Tolerant of the many shapes yt-dlp emits: numeric fields may be JSON numbers,
/// strings, null or absent; codec fields use `"none"` / `""` to signal absence.
pub struct JsonMediaInfoParser;

impl MediaInfoParser for JsonMediaInfoParser {
    fn parse(&self, json: &str) -> Result<MediaInfo, ParseError> {
        let root = parse_value(json)?;
        if !root.is_object() {
            return Err(ParseError::InvalidData(
                "yt-dlp 返回的数据不是有效的 JSON 对象".to_string(),
            ));
        }

        let id = str_field(&root, "id").unwrap_or_else(|| "-".to_string());
        let title = str_field(&root, "title").unwrap_or_else(|| id.clone());
        let webpage = str_field(&root, "webpage_url").unwrap_or_default();
        let uploader = str_field(&root, "uploader").or_else(|| str_field(&root, "channel"));

        // Live detection: "is_live" is authoritative; "live_status" is a fallback
        // that also tells apart running (is_live), scheduled (is_upcoming) and
        // finished (was_live/post_live) streams. Finished streams are ordinary
        // videos and must not enter live mode.
        let live_status = str_field(&root, "live_status");
        let is_live = bool_field(&root, "is_live") == Some(true)
            || live_status
                .as_deref()
                .is_some_and(|status| status.eq_ignore_ascii_case("is_live"));
        let is_upcoming = live_status
            .as_deref()
            .is_some_and(|status| status.eq_ignore_ascii_case("is_upcoming"))
            || bool_field(&root, "is_upcoming") == Some(true);

        let duration = num_field(&root, "duration")
            .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok());

        let mut videos = Vec::new();
        let mut audios = Vec::new();

        if let Some(Value::Array(formats)) = root.get("formats") {
            for format in formats {
                if !format.is_object() {
                    continue;
                }

                let Some(format_id) = str_field(format, "format_id") else {
                    continue;
                };

                let has_video = is_present(format, "vcodec");
                let has_audio = is_present(format, "acodec");
                if !has_video && !has_audio {
                    continue;
                }

                let container = str_field(format, "ext").unwrap_or_else(|| "-".to_string());

                if has_video {
                    let codec = str_field(format, "vcodec").unwrap_or_else(|| "-".to_string());
                    let audio_codec = has_audio.then(|| str_field(format, "acodec")).flatten();
                    videos.push(VideoSource::new(
                        format_id,
                        container,
                        codec,
                        has_audio,
                        audio_codec,
                        num_field(format, "width").map(|value| value as i64),
                        num_field(format, "height").map(|value| value as i64),
                        num_field(format, "fps"),
                        num_field(format, "vbr")
                            .or_else(|| num_field(format, "tbr"))
                            .or_else(|| num_field(format, "abr")),
                    ));
                } else {
                    audios.push(AudioSource::new(
                        format_id,
                        container,
                        str_field(format, "acodec").unwrap_or_else(|| "-".to_string()),
                        num_field(format, "abr").or_else(|| num_field(format, "tbr")),
                    ));
                }
            }
        }

        // Prefer the most attractive source first (height, then bitrate, then fps).
        videos.sort_by(|a, b| {
            let by_height = b.height.unwrap_or(0).cmp(&a.height.unwrap_or(0));
            if by_height != std::cmp::Ordering::Equal {
                return by_height;
            }

            let by_bitrate = compare_desc(a.bitrate_kbps, b.bitrate_kbps);
            if by_bitrate != std::cmp::Ordering::Equal {
                return by_bitrate;
            }

            compare_desc(a.fps, b.fps)
        });

        audios.sort_by(|a, b| compare_desc(a.bitrate_kbps, b.bitrate_kbps));

        // An upcoming stream exposes no formats yet; keep it parseable so the UI
        // can prompt instead of reporting a generic parse failure.
        if videos.is_empty() && audios.is_empty() && !is_live && !is_upcoming {
            return Err(ParseError::InvalidData(
                "未解析到任何可用的视频/音频格式".to_string(),
            ));
        }

        let mut media = MediaInfo::new(
            id,
            title,
            webpage,
            duration,
            uploader,
            best_thumbnail(&root),
            videos,
            audios,
        );
        media.is_live = is_live;
        media.is_upcoming = is_upcoming;

        Ok(media)
    }

    fn parse_playlist(&self, json: &str) -> Result<Option<PlaylistInfo>, ParseError> {
        let root = parse_value(json)?;
        if !root.is_object() {
            return Ok(None);
        }

        // yt-dlp marks playlist containers with "_type": "playlist".
        let is_playlist = str_field(&root, "_type")
            .is_some_and(|value| value.eq_ignore_ascii_case("playlist"));
        if !is_playlist {
            return Ok(None);
        }

        let Some(Value::Array(entries)) = root.get("entries") else {
            return Ok(None);
        };

        let items = entries
            .iter()
            .filter(|entry| entry.is_object())
            .filter_map(|entry| {
                // Flat listings expose the canonical page URL as "webpage_url"
                // and often as "url" too.
                let url = str_field(entry, "webpage_url").or_else(|| str_field(entry, "url"))?;
                let title = str_field(entry, "title").unwrap_or_else(|| url.clone());
                Some(PlaylistEntry { url, title })
            })
            .collect();

        Ok(Some(PlaylistInfo {
            id: str_field(&root, "id").unwrap_or_else(|| "-".to_string()),
            title: str_field(&root, "title").unwrap_or_else(|| "播放列表".to_string()),
            entries: items,
        }))
    }
}

fn parse_value(json: &str) -> Result<Value, ParseError> {
    serde_json::from_str(json).map_err(|error| ParseError::InvalidJson(error.to_string()))
}

/// Prefers the top-level `thumbnail` URL, otherwise the largest `thumbnails` entry.
fn best_thumbnail(root: &Value) -> Option<String> {
    if let Some(direct) = str_field(root, "thumbnail") {
        return Some(direct);
    }

    let Some(Value::Array(thumbnails)) = root.get("thumbnails") else {
        return None;
    };

    let mut best: Option<String> = None;
    let mut best_area = -1.0_f64;
    for thumbnail in thumbnails {
        let Some(url) = str_field(thumbnail, "url") else {
            continue;
        };

        let area = num_field(thumbnail, "width").unwrap_or(0.0)
            * num_field(thumbnail, "height").unwrap_or(0.0);
        if area >= best_area {
            best_area = area;
            best = Some(url);
        }
    }

    best
}

/// Descending comparison that treats `None`/NaN as the lowest value.
fn compare_desc(left: Option<f64>, right: Option<f64>) -> std::cmp::Ordering {
    right
        .unwrap_or(0.0)
        .partial_cmp(&left.unwrap_or(0.0))
        .unwrap_or(std::cmp::Ordering::Equal)
}

/// True when the named string field is present, non-empty and not `"none"`.
fn is_present(object: &Value, name: &str) -> bool {
    match object.get(name) {
        Some(Value::String(value)) => {
            !value.is_empty() && !value.eq_ignore_ascii_case("none")
        }
        _ => false,
    }
}

/// Non-empty string field, or `None`.
fn str_field(object: &Value, name: &str) -> Option<String> {
    let value = object.get(name)?.as_str()?;
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Numeric field, accepting JSON numbers and numeric strings.
fn num_field(object: &Value, name: &str) -> Option<f64> {
    match object.get(name)? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Boolean field, accepting JSON booleans and `"true"` / `"false"` strings.
fn bool_field(object: &Value, name: &str) -> Option<bool> {
    match object.get(name)? {
        Value::Bool(value) => Some(*value),
        Value::String(text) => match text.trim().to_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}