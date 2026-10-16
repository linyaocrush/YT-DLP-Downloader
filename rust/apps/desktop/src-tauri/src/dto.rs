//! Serializable shapes exchanged with the Svelte frontend.
//!
//! The core models are intentionally not `Serialize` (they belong to the
//! UI-independent domain); these DTOs add the derived display strings the old
//! WPF bindings exposed (`Resolution`, `FpsText`, `DurationText`, ...) so the
//! frontend stays a pure presenter.

use serde::{Deserialize, Serialize};

use ytdlp_downloader_core::{
    AudioSource, DownloadKind, DownloadUpdate, MediaInfo, PlaylistInfo, ResolutionPreference,
    VideoSource,
};

// ---------------------------------------------------------------------------
// Media metadata
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSourceDto {
    pub format_id: String,
    pub resolution: String,
    pub fps_text: String,
    pub bitrate_text: String,
    pub codec: String,
    pub kind: String,
    pub has_audio: bool,
    pub container: String,
}

impl From<&VideoSource> for VideoSourceDto {
    fn from(source: &VideoSource) -> Self {
        Self {
            format_id: source.format_id.clone(),
            resolution: source.resolution(),
            fps_text: source.fps_text(),
            bitrate_text: source.bitrate_text(),
            codec: source.codec.clone(),
            kind: source.kind().to_string(),
            has_audio: source.has_audio,
            container: source.container.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSourceDto {
    pub format_id: String,
    pub bitrate_text: String,
    pub codec: String,
    pub container: String,
}

impl From<&AudioSource> for AudioSourceDto {
    fn from(source: &AudioSource) -> Self {
        Self {
            format_id: source.format_id.clone(),
            bitrate_text: source.bitrate_text(),
            codec: source.codec.clone(),
            container: source.container.clone(),
        }
    }
}

/// Everything the download page needs to render one parsed video.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfoDto {
    pub id: String,
    pub title: String,
    pub webpage_url: String,
    pub duration_text: String,
    pub uploader: Option<String>,
    pub thumbnail_url: Option<String>,
    pub videos: Vec<VideoSourceDto>,
    pub audios: Vec<AudioSourceDto>,
    pub is_live: bool,
    pub is_upcoming: bool,
}

impl From<&MediaInfo> for MediaInfoDto {
    fn from(info: &MediaInfo) -> Self {
        Self {
            id: info.id.clone(),
            title: info.title.clone(),
            webpage_url: info.webpage_url.clone(),
            duration_text: info.duration_text(),
            uploader: info.uploader.clone(),
            thumbnail_url: info.thumbnail_url.clone(),
            videos: info.videos.iter().map(VideoSourceDto::from).collect(),
            audios: info.audios.iter().map(AudioSourceDto::from).collect(),
            is_live: info.is_live,
            is_upcoming: info.is_upcoming,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntryDto {
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInfoDto {
    pub id: String,
    pub title: String,
    pub entries: Vec<PlaylistEntryDto>,
}

impl From<&PlaylistInfo> for PlaylistInfoDto {
    fn from(playlist: &PlaylistInfo) -> Self {
        Self {
            id: playlist.id.clone(),
            title: playlist.title.clone(),
            entries: playlist
                .entries
                .iter()
                .map(|entry| PlaylistEntryDto {
                    url: entry.url.clone(),
                    title: entry.title.clone(),
                })
                .collect(),
        }
    }
}

/// Result of parsing a URL that may be a single video or a playlist. Exactly one
/// field is populated.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResultDto {
    pub media: Option<MediaInfoDto>,
    pub playlist: Option<PlaylistInfoDto>,
}

// ---------------------------------------------------------------------------
// Download
// ---------------------------------------------------------------------------

/// Manual source selection, forwarded verbatim so the format expression is built
/// in Rust (the exact C# rules live in `YtDlpFormat`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualSelectionDto {
    pub video_format_id: Option<String>,
    pub video_has_audio: bool,
    pub audio_format_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequestDto {
    pub url: String,
    /// True for auto selection; false to use `manual`.
    pub auto: bool,
    pub kind: DownloadKind,
    pub resolution: ResolutionPreference,
    pub manual: Option<ManualSelectionDto>,
    /// Final on-disk directory; blank falls back to the default download folder.
    pub output_directory: String,
    /// `None` lets yt-dlp keep its own default template.
    pub output_template: Option<String>,
    pub write_thumbnail: bool,
    pub live: bool,
}

/// One incremental progress report, emitted as an event while a download runs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdateDto {
    pub line: Option<String>,
    pub percent: Option<f64>,
    pub output_path: Option<String>,
    pub speed: Option<String>,
    pub eta: Option<String>,
}

impl From<DownloadUpdate> for DownloadUpdateDto {
    fn from(update: DownloadUpdate) -> Self {
        Self {
            line: update.line,
            percent: update.percent,
            output_path: update.output_path,
            speed: update.speed,
            eta: update.eta,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOutcomeDto {
    pub success: bool,
    pub cancelled: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Environment / settings derived state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtDlpStatusDto {
    /// Resolved executable path, or `null` when yt-dlp could not be located.
    pub path: Option<String>,
    pub ready: bool,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YtDlpDetectionDto {
    pub resolved: Option<String>,
    pub version: Option<String>,
    pub ready: bool,
    pub show_download_hint: bool,
    pub status_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegStatusDto {
    pub located_in_path: Option<String>,
    pub configured_path: String,
    pub configured_exists: bool,
    pub ready: bool,
    pub show_manual_controls: bool,
    pub show_download_hint: bool,
    pub status_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieFileDto {
    pub path: String,
    pub name: String,
}

/// Cookie folder contents plus the hint shown under the download-page selector.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieListingDto {
    pub folder: String,
    pub exists: bool,
    pub needs_setup: bool,
    pub hint: String,
    pub files: Vec<CookieFileDto>,
}

/// Short proxy indicator for the download page, mirroring
/// `DownloadViewModel.RefreshProxyStatus`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyIndicatorDto {
    pub will_apply: bool,
    pub text: String,
    pub detail: String,
    pub proxy_url: Option<String>,
}
