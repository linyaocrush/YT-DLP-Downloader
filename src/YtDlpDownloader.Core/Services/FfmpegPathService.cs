namespace YtDlpDownloader.Core.Services;

public interface IFfmpegPathService
{
    /// <summary>Scans every directory on the PATH environment variable for ffmpeg.</summary>
    string? LocateInPath();

    /// <summary>Returns the manually configured path when it still exists on disk, otherwise null.</summary>
    string? GetConfiguredPath();

    /// <summary>
    /// Path to hand to yt-dlp via --ffmpeg-location, or null when nothing should be passed.
    /// ffmpeg on PATH is resolved by yt-dlp itself, so only a manual (non-PATH) path is returned.
    /// </summary>
    string? GetExplicitPathForYtDlp();
}

public sealed class FfmpegPathService : IFfmpegPathService
{
    private static readonly string[] CandidateFileNames = { "ffmpeg.exe" };

    private readonly ISettingsService _settings;

    public FfmpegPathService(ISettingsService settings) => _settings = settings;

    public string? LocateInPath()
    {
        var path = Environment.GetEnvironmentVariable("PATH");
        if (string.IsNullOrWhiteSpace(path))
            return null;

        foreach (var rawDir in path.Split(Path.PathSeparator))
        {
            if (string.IsNullOrWhiteSpace(rawDir))
                continue;

            string dir;
            try
            {
                dir = Path.GetFullPath(rawDir);
            }
            catch
            {
                continue;
            }

            foreach (var name in CandidateFileNames)
            {
                try
                {
                    var candidate = Path.Combine(dir, name);
                    if (File.Exists(candidate))
                        return candidate;
                }
                catch
                {
                    // Skip directories we cannot inspect.
                }
            }
        }

        return null;
    }

    public string? GetConfiguredPath()
    {
        var configured = _settings.Settings.FfmpegPath?.Trim();
        return !string.IsNullOrEmpty(configured) && File.Exists(configured)
            ? configured
            : null;
    }

    public string? GetExplicitPathForYtDlp()
        => LocateInPath() is not null ? null : GetConfiguredPath();
}
