//! User-editable application settings, persisted as JSON.
//!
//! The JSON shape intentionally matches the C# original (`JsonStringEnumConverter`
//! + PascalCase property names) so a `settings.json` written by either
//! implementation can be read by the other.

use serde::{Deserialize, Serialize};

/// User-editable application settings, persisted as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettings {
    pub yt_dlp_path: String,

    /// Optional ffmpeg executable passed to yt-dlp via `--ffmpeg-location`.
    pub ffmpeg_path: String,

    pub download_directory: String,

    /// Optional Netscape-format cookies file passed to yt-dlp via `--cookies`.
    pub cookie_file: String,

    /// Folder scanned for cookie files offered in the download view.
    pub cookie_folder: String,

    /// Number of fragments downloaded concurrently (`--concurrent-fragments`).
    pub download_threads: i32,

    /// How the output file name is produced (template fields or a fixed name).
    pub output_name_mode: OutputNameMode,

    /// Selected yt-dlp template fields (space separated, without extension).
    pub output_template: String,

    /// Fixed base file name used in [`OutputNameMode::Fixed`].
    pub output_file_name: String,

    /// What to download: combined media, video only or audio only.
    pub download_kind: DownloadKind,

    /// Preferred maximum video resolution (auto mode only).
    pub resolution: ResolutionPreference,

    /// When true, save the cover as a separate image (`--write-thumbnail`).
    pub write_thumbnail: bool,

    /// Master switch: when false the proxy is never used, regardless of the site list.
    pub proxy_enabled: bool,

    /// Protocol prefix used when building the yt-dlp `--proxy` value.
    pub proxy_protocol: ProxyProtocol,

    /// Proxy host or IP address.
    pub proxy_host: String,

    /// Proxy port, kept as text so it can be edited before being validated.
    pub proxy_port: String,

    /// Whether [`Self::proxy_sites`] is an allow list or a deny list.
    pub proxy_list_mode: ProxyListMode,

    /// Websites the proxy rule applies to (host names or URL prefixes).
    pub proxy_sites: Vec<String>,

    /// Light/dark appearance; [`AppTheme::System`] follows the OS setting.
    pub theme: AppTheme,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            yt_dlp_path: String::new(),
            ffmpeg_path: String::new(),
            download_directory: String::new(),
            cookie_file: String::new(),
            cookie_folder: String::new(),
            download_threads: 4,
            output_name_mode: OutputNameMode::Template,
            output_template: "%(title)s".to_string(),
            output_file_name: String::new(),
            download_kind: DownloadKind::Default,
            resolution: ResolutionPreference::Best,
            write_thumbnail: false,
            proxy_enabled: false,
            proxy_protocol: ProxyProtocol::Http,
            proxy_host: String::new(),
            proxy_port: String::new(),
            proxy_list_mode: ProxyListMode::Whitelist,
            proxy_sites: Vec::new(),
            theme: AppTheme::System,
        }
    }
}

/// Preferred video resolution used when yt-dlp picks the format automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ResolutionPreference {
    /// No resolution constraint; pick the highest available quality.
    Best,
    /// Only formats taller than 4K (2160p).
    Above4K,
    /// At most 4K (2160p).
    UpTo4K,
    /// At most 2K (1440p).
    UpTo2K,
    /// At most 1080p.
    UpTo1080P,
    /// At most 720p.
    UpTo720P,
    /// At most 360p.
    UpTo360P,
}

/// Which stream(s) the download should contain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DownloadKind {
    /// Best video together with the best audio (default).
    Default,
    /// Video stream only, without audio.
    VideoOnly,
    /// Audio stream only.
    AudioOnly,
}

/// How the downloaded file name is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum OutputNameMode {
    /// Build the name from the selected yt-dlp template fields.
    Template,
    /// Use a user-supplied fixed base name.
    Fixed,
}

/// Protocol prefix for the configured proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProxyProtocol {
    /// Plain HTTP proxy (`http://`).
    Http,
    /// HTTPS proxy (`https://`).
    Https,
    /// SOCKS5 proxy (`socks5://`).
    Socks5,
}

/// How the website list is interpreted by the proxy rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ProxyListMode {
    /// Only listed websites use the proxy.
    Whitelist,
    /// All websites except the listed ones use the proxy.
    Blacklist,
}

/// Application colour scheme selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AppTheme {
    /// Light palette.
    Light,
    /// Dark palette.
    Dark,
    /// Follow the OS light/dark app setting.
    System,
}