namespace YtDlpDownloader.Core.Models;

/// <summary>Builds automatic yt-dlp format selection expressions from user-facing options.</summary>
public static class YtDlpFormat
{
    /// <summary>Fallback expression: best video+audio, or best single file when no merge is possible.</summary>
    public const string AutoFormatExpression = "bv*+ba/b";

    public static string BuildExpression(DownloadKind kind, ResolutionPreference preference)
    {
        var filter = kind == DownloadKind.AudioOnly ? null : ResolutionFilter(preference);

        return kind switch
        {
            DownloadKind.VideoOnly => filter is null ? "bv" : $"bv{filter}",
            DownloadKind.AudioOnly => "ba",
            _ => filter is null ? AutoFormatExpression : $"bv*{filter}+ba/b{filter}",
        };
    }

    private static string? ResolutionFilter(ResolutionPreference preference) => preference switch
    {
        ResolutionPreference.Above4K => "[height>2160]",
        ResolutionPreference.UpTo4K => "[height<=2160]",
        ResolutionPreference.UpTo2K => "[height<=1440]",
        ResolutionPreference.UpTo1080P => "[height<=1080]",
        ResolutionPreference.UpTo720P => "[height<=720]",
        ResolutionPreference.UpTo360P => "[height<=360]",
        _ => null,
    };
}
