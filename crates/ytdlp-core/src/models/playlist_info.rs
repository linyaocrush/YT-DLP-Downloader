//! Playlist (video queue) models.

/// One video entry of a playlist, as listed by yt-dlp (flat listing).
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistEntry {
    pub index: usize,
    pub url: String,
    pub title: String,
    pub thumbnail_url: Option<String>,
}

/// A playlist (video queue) together with the videos it contains.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistInfo {
    pub id: String,
    pub title: String,
    pub entries: Vec<PlaylistEntry>,
}
