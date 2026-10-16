//! Error type returned by Tauri commands.
//!
//! Tauri rejects the JS promise with this value, so the frontend can tell a user
//! cancellation apart from a normal failure without string matching.

use serde::Serialize;

use ytdlp_downloader_core::YtDlpError;

/// A command failure, tagged so the frontend can branch on it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// `"cancelled"`, `"message"` or `"io"`.
    pub kind: String,
    pub message: String,
}

impl CommandError {
    pub fn cancelled() -> Self {
        Self {
            kind: "cancelled".to_string(),
            message: "操作已取消".to_string(),
        }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self {
            kind: "message".to_string(),
            message: message.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self {
            kind: "io".to_string(),
            message: message.into(),
        }
    }
}

impl From<YtDlpError> for CommandError {
    fn from(error: YtDlpError) -> Self {
        match error {
            YtDlpError::Cancelled => Self::cancelled(),
            YtDlpError::Message(message) => Self::message(message),
            YtDlpError::Io(error) => Self::io(error.to_string()),
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
