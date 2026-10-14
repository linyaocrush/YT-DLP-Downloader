namespace YtDlpDownloader.Core.Models;

/// <summary>One video entry of a playlist, as listed by yt-dlp (flat listing).</summary>
public sealed record PlaylistEntry(string Url, string Title);

/// <summary>A playlist (video queue) together with the videos it contains.</summary>
public sealed record PlaylistInfo(
    string Id,
    string Title,
    IReadOnlyList<PlaylistEntry> Entries);
