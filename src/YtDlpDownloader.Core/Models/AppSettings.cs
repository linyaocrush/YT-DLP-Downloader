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

    /// <summary>How the output file name is produced (template fields or a fixed name).</summary>
    public OutputNameMode OutputNameMode { get; set; } = OutputNameMode.Template;

    /// <summary>Selected yt-dlp template fields (space separated, without extension).</summary>
    public string OutputTemplate { get; set; } = "%(title)s";

    /// <summary>Fixed base file name used in <see cref="OutputNameMode.Fixed"/> (extension is appended automatically).</summary>
    public string OutputFileName { get; set; } = string.Empty;
}

/// <summary>How the downloaded file name is built.</summary>
public enum OutputNameMode
{
    /// <summary>Build the name from the selected yt-dlp template fields.</summary>
    Template,

    /// <summary>Use a user-supplied fixed base name.</summary>
    Fixed,
}
