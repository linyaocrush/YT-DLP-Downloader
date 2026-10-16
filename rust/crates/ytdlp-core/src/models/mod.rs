//! Domain models ported from `YtDlpDownloader.Core.Models`.

pub mod app_settings;
pub mod media_info;
pub mod output_template;
pub mod playlist_info;
pub mod proxy_rules;
pub mod ytdlp_format;

pub use app_settings::{
    AppSettings, AppTheme, DownloadKind, OutputNameMode, ProxyListMode, ProxyProtocol,
    ResolutionPreference,
};
pub use media_info::{AudioSource, MediaInfo, VideoSource};
pub use playlist_info::{PlaylistEntry, PlaylistInfo};
pub use proxy_rules::ProxyRules;
pub use ytdlp_format::YtDlpFormat;