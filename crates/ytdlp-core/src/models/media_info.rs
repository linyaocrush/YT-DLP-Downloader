//! Parsed media metadata models (`VideoSource`, `AudioSource`, `MediaInfo`).

use std::time::Duration;

/// A single video-bearing source (pure video, or combined video + audio).
#[derive(Debug, Clone, PartialEq)]
pub struct VideoSource {
    pub format_id: String,
    pub container: String,
    pub codec: String,
    pub has_audio: bool,
    pub audio_codec: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub fps: Option<f64>,
    pub bitrate_kbps: Option<f64>,
}

impl VideoSource {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        format_id: impl Into<String>,
        container: impl Into<String>,
        codec: impl Into<String>,
        has_audio: bool,
        audio_codec: Option<String>,
        width: Option<i64>,
        height: Option<i64>,
        fps: Option<f64>,
        bitrate_kbps: Option<f64>,
    ) -> Self {
        Self {
            format_id: format_id.into(),
            container: container.into(),
            codec: codec.into(),
            has_audio,
            audio_codec,
            width,
            height,
            fps,
            bitrate_kbps,
        }
    }

    /// `"{width}x{height}"`, or `-` when either dimension is missing.
    pub fn resolution(&self) -> String {
        match (self.width, self.height) {
            (Some(width), Some(height)) => format!("{width}x{height}"),
            _ => "-".to_string(),
        }
    }

    /// Frame rate as a short string; whole numbers lose their decimal part.
    pub fn fps_text(&self) -> String {
        match self.fps {
            None => "-".to_string(),
            Some(fps) => {
                let rounded = fps.round();
                if (fps - rounded).abs() < 0.01 {
                    format!("{}", rounded as i64)
                } else {
                    trim_trailing_zero(format!("{fps:.1}"))
                }
            }
        }
    }

    /// Rounded bitrate, e.g. `4500 kbps`, or `-` when unknown.
    pub fn bitrate_text(&self) -> String {
        format_bitrate(self.bitrate_kbps)
    }

    /// `音视频合一` when the source also carries audio, otherwise `纯视频`.
    pub fn kind(&self) -> &'static str {
        if self.has_audio {
            "音视频合一"
        } else {
            "纯视频"
        }
    }
}

/// A single audio-only source.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioSource {
    pub format_id: String,
    pub container: String,
    pub codec: String,
    pub bitrate_kbps: Option<f64>,
}

impl AudioSource {
    pub fn new(
        format_id: impl Into<String>,
        container: impl Into<String>,
        codec: impl Into<String>,
        bitrate_kbps: Option<f64>,
    ) -> Self {
        Self {
            format_id: format_id.into(),
            container: container.into(),
            codec: codec.into(),
            bitrate_kbps,
        }
    }

    /// Rounded bitrate, e.g. `128 kbps`, or `-` when unknown.
    pub fn bitrate_text(&self) -> String {
        format_bitrate(self.bitrate_kbps)
    }
}

/// Parsed metadata for one URL.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub id: String,
    pub title: String,
    pub webpage_url: String,
    pub duration: Option<Duration>,
    pub uploader: Option<String>,
    pub thumbnail_url: Option<String>,
    pub videos: Vec<VideoSource>,
    pub audios: Vec<AudioSource>,
    /// True while the source is an ongoing live stream.
    pub is_live: bool,
    /// True when the source is a scheduled (not yet started) stream.
    pub is_upcoming: bool,
}

impl MediaInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        webpage_url: impl Into<String>,
        duration: Option<Duration>,
        uploader: Option<String>,
        thumbnail_url: Option<String>,
        videos: Vec<VideoSource>,
        audios: Vec<AudioSource>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            webpage_url: webpage_url.into(),
            duration,
            uploader,
            thumbnail_url,
            videos,
            audios,
            is_live: false,
            is_upcoming: false,
        }
    }

    /// Duration as `m:ss` (or `h:mm:ss`), or `-` when unknown.
    pub fn duration_text(&self) -> String {
        match self.duration {
            None => "-".to_string(),
            Some(duration) => {
                let total = duration.as_secs();
                let hours = total / 3600;
                let minutes = (total % 3600) / 60;
                let seconds = total % 60;
                if hours >= 1 {
                    format!("{hours}:{minutes:02}:{seconds:02}")
                } else {
                    format!("{minutes}:{seconds:02}")
                }
            }
        }
    }
}

fn format_bitrate(bitrate_kbps: Option<f64>) -> String {
    match bitrate_kbps {
        None => "-".to_string(),
        Some(value) => format!("{} kbps", value.round() as i64),
    }
}

/// Drops a trailing `.0` from a one-decimal formatted float (`24.0` -> `24`).
fn trim_trailing_zero(text: String) -> String {
    text.strip_suffix(".0").map(str::to_string).unwrap_or(text)
}
