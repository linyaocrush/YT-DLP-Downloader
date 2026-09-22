namespace YtDlpDownloader.Core.Models;

/// <summary>User-editable application settings, persisted as JSON.</summary>
public sealed class AppSettings
{
    public string YtDlpPath { get; set; } = string.Empty;

    /// <summary>Optional ffmpeg executable passed to yt-dlp via --ffmpeg-location.</summary>
    public string FfmpegPath { get; set; } = string.Empty;

    public string DownloadDirectory { get; set; } = string.Empty;

    /// <summary>Optional Netscape-format cookies file passed to yt-dlp via --cookies.</summary>
    public string CookieFile { get; set; } = string.Empty;

    /// <summary>Folder scanned for cookie files offered in the download view.</summary>
    public string CookieFolder { get; set; } = string.Empty;

    /// <summary>Number of fragments downloaded concurrently (yt-dlp --concurrent-fragments).</summary>
    public int DownloadThreads { get; set; } = 4;

    /// <summary>How the output file name is produced (template fields or a fixed name).</summary>
    public OutputNameMode OutputNameMode { get; set; } = OutputNameMode.Template;

    /// <summary>Selected yt-dlp template fields (space separated, without extension).</summary>
    public string OutputTemplate { get; set; } = "%(title)s";

    /// <summary>Fixed base file name used in <see cref="OutputNameMode.Fixed"/> (extension is appended automatically).</summary>
    public string OutputFileName { get; set; } = string.Empty;

    /// <summary>What to download: combined media, video only, or audio only.</summary>
    public DownloadKind DownloadKind { get; set; } = DownloadKind.Default;

    /// <summary>Preferred maximum video resolution (auto mode only).</summary>
    public ResolutionPreference Resolution { get; set; } = ResolutionPreference.Best;
}

/// <summary>Preferred video resolution used when yt-dlp picks the format automatically.</summary>
public enum ResolutionPreference
{
    /// <summary>No resolution constraint; pick the highest available quality.</summary>
    Best,

    /// <summary>Only formats taller than 4K (2160p).</summary>
    Above4K,

    /// <summary>At most 4K (2160p).</summary>
    UpTo4K,

    /// <summary>At most 2K (1440p).</summary>
    UpTo2K,

    /// <summary>At most 1080p.</summary>
    UpTo1080P,

    /// <summary>At most 720p.</summary>
    UpTo720P,

    /// <summary>At most 360p.</summary>
    UpTo360P,
}

/// <summary>Which stream(s) the download should contain.</summary>
public enum DownloadKind
{
    /// <summary>Download the best video together with the best audio (default).</summary>
    Default,

    /// <summary>Download the video stream only, without audio.</summary>
    VideoOnly,

    /// <summary>Download the audio stream only.</summary>
    AudioOnly,
}

/// <summary>How the downloaded file name is built.</summary>
public enum OutputNameMode
{
    /// <summary>Build the name from the selected yt-dlp template fields.</summary>
    Template,

    /// <summary>Use a user-supplied fixed base name.</summary>
    Fixed,
}
