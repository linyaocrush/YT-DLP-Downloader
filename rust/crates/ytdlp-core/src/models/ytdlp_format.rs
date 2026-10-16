//! Builds automatic yt-dlp format-selection expressions from user options.

use super::app_settings::{DownloadKind, ResolutionPreference};

/// Builds automatic yt-dlp format selection expressions from user-facing options.
pub struct YtDlpFormat;

impl YtDlpFormat {
    /// Fallback expression: best video+audio, or best single file when no merge is possible.
    pub const AUTO_FORMAT_EXPRESSION: &'static str = "bv*+ba/b";

    pub fn build_expression(kind: DownloadKind, preference: ResolutionPreference) -> String {
        let filter = if kind == DownloadKind::AudioOnly {
            None
        } else {
            Self::resolution_filter(preference)
        };

        match kind {
            DownloadKind::VideoOnly => match filter {
                None => "bv".to_string(),
                Some(filter) => format!("bv{filter}"),
            },
            DownloadKind::AudioOnly => "ba".to_string(),
            DownloadKind::Default => match filter {
                None => Self::AUTO_FORMAT_EXPRESSION.to_string(),
                Some(filter) => format!("bv*{filter}+ba/b{filter}"),
            },
        }
    }

    fn resolution_filter(preference: ResolutionPreference) -> Option<&'static str> {
        match preference {
            ResolutionPreference::Above4K => Some("[height>2160]"),
            ResolutionPreference::UpTo4K => Some("[height<=2160]"),
            ResolutionPreference::UpTo2K => Some("[height<=1440]"),
            ResolutionPreference::UpTo1080P => Some("[height<=1080]"),
            ResolutionPreference::UpTo720P => Some("[height<=720]"),
            ResolutionPreference::UpTo360P => Some("[height<=360]"),
            ResolutionPreference::Best => None,
        }
    }
}