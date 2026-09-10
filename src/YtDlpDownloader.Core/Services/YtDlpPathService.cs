namespace YtDlpDownloader.Core.Services;

public interface IYtDlpPathService
{
    /// <summary>Scans every directory on the PATH environment variable for yt-dlp.</summary>
    string? LocateInPath();

    /// <summary>
    /// Effective executable: returns the configured path when it still exists on disk,
    /// otherwise falls back to a PATH search (persisting a hit into settings).
    /// </summary>
    string? Resolve();
}

public sealed class YtDlpPathService : IYtDlpPathService
{
    private static readonly string[] CandidateFileNames = { "yt-dlp.exe" };

    private readonly ISettingsService _settings;

    public YtDlpPathService(ISettingsService settings) => _settings = settings;

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

    public string? Resolve()
    {
        var configured = _settings.Settings.YtDlpPath?.Trim();
        if (!string.IsNullOrEmpty(configured) && File.Exists(configured))
            return configured;

        var found = LocateInPath();
        if (found is not null)
        {
            _settings.Settings.YtDlpPath = found;
            _settings.Save();
            return found;
        }

        return null;
    }
}
