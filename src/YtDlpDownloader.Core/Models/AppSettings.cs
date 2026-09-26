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

    /// <summary>Master switch: when false the proxy is never used, regardless of the site list.</summary>
    public bool ProxyEnabled { get; set; }

    /// <summary>Protocol prefix used when building the yt-dlp --proxy value.</summary>
    public ProxyProtocol ProxyProtocol { get; set; } = ProxyProtocol.Http;

    /// <summary>Proxy host or IP address.</summary>
    public string ProxyHost { get; set; } = string.Empty;

    /// <summary>Proxy port, kept as text so it can be edited freely before being validated.</summary>
    public string ProxyPort { get; set; } = string.Empty;

    /// <summary>Whether <see cref="ProxySites"/> is treated as an allow list or a deny list.</summary>
    public ProxyListMode ProxyListMode { get; set; } = ProxyListMode.Whitelist;

    /// <summary>Websites the proxy rule applies to (host names or URL prefixes).</summary>
    public List<string> ProxySites { get; set; } = new();
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

/// <summary>Protocol prefix for the configured proxy.</summary>
public enum ProxyProtocol
{
    /// <summary>Plain HTTP proxy (http://).</summary>
    Http,

    /// <summary>HTTPS proxy (https://).</summary>
    Https,

    /// <summary>SOCKS5 proxy (socks5://).</summary>
    Socks5,
}

/// <summary>How the website list is interpreted by the proxy rule.</summary>
public enum ProxyListMode
{
    /// <summary>Only the listed websites use the proxy; everything else connects directly.</summary>
    Whitelist,

    /// <summary>All websites except the listed ones use the proxy.</summary>
    Blacklist,
}
