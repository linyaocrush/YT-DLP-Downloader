//! Playlist (video queue) models.

/// One video entry of a playlist, as listed by yt-dlp (flat listing).
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistEntry {
    pub url: String,
    pub title: String,
}

/// A playlist (video queue) together with the videos it contains.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistInfo {
    pub id: String,
    pub title: String,
    pub entries: Vec<PlaylistEntry>,
}