namespace YtDlpDownloader.Core.Models;

/// <summary>User-editable application settings, persisted as JSON.</summary>
public sealed class AppSettings
{
    public string YtDlpPath { get; set; } = string.Empty;

    public string DownloadDirectory { get; set; } = string.Empty;
}
