namespace YtDlpDownloader.Core.Models;

/// <summary>User-editable application settings, persisted as JSON.</summary>
public sealed class AppSettings
{
    public string YtDlpPath { get; set; } = string.Empty;

    public string DownloadDirectory { get; set; } = string.Empty;

    /// <summary>Optional Netscape-format cookies file passed to yt-dlp via --cookies.</summary>
    public string CookieFile { get; set; } = string.Empty;

    /// <summary>Number of fragments downloaded concurrently (yt-dlp --concurrent-fragments).</summary>
    public int DownloadThreads { get; set; } = 4;
}
