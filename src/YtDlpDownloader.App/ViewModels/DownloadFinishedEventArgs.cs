namespace YtDlpDownloader.App.ViewModels;

/// <summary>Outcome of a finished download attempt, used for taskbar state and notifications.</summary>
public sealed class DownloadFinishedEventArgs : EventArgs
{
    public DownloadFinishedEventArgs(
        bool success,
        bool cancelled,
        string? error,
        string? outputPath,
        string title,
        string url)
    {
        Success = success;
        Cancelled = cancelled;
        Error = error;
        OutputPath = outputPath;
        Title = title;
        Url = url;
    }

    public bool Success { get; }

    public bool Cancelled { get; }

    /// <summary>Failure message, or null when the download succeeded or was cancelled.</summary>
    public string? Error { get; }

    /// <summary>Final file path reported by yt-dlp, when known.</summary>
    public string? OutputPath { get; }

    /// <summary>Parsed media title, when known.</summary>
    public string Title { get; }

    /// <summary>The source URL that was downloaded.</summary>
    public string Url { get; }
}