namespace YtDlpDownloader.Core.Models;

/// <summary>A single video-bearing source (pure video, or combined video+audio).</summary>
public sealed record VideoSource(
    string FormatId,
    string Container,
    string Codec,
    bool HasAudio,
    string? AudioCodec,
    int? Width,
    int? Height,
    double? Fps,
    double? BitrateKbps)
{
    public string Resolution => Width.HasValue && Height.HasValue ? $"{Width}x{Height}" : "-";

    public string FpsText => Fps is null
        ? "-"
        : Math.Abs(Fps.Value - Math.Round(Fps.Value)) < 0.01
            ? ((int)Math.Round(Fps.Value)).ToString()
            : Fps.Value.ToString("0.#");

    public string BitrateText => BitrateKbps is null ? "-" : $"{Math.Round(BitrateKbps.Value):0} kbps";

    public string Kind => HasAudio ? "音视频合一" : "纯视频";
}

/// <summary>A single audio-only source.</summary>
public sealed record AudioSource(
    string FormatId,
    string Container,
    string Codec,
    double? BitrateKbps)
{
    public string BitrateText => BitrateKbps is null ? "-" : $"{Math.Round(BitrateKbps.Value):0} kbps";
}

/// <summary>Parsed metadata for one URL.</summary>
public sealed record MediaInfo(
    string Id,
    string Title,
    string WebpageUrl,
    TimeSpan? Duration,
    string? Uploader,
    string? ThumbnailUrl,
    IReadOnlyList<VideoSource> Videos,
    IReadOnlyList<AudioSource> Audios)
{
    public string DurationText => Duration is null
        ? "-"
        : Duration.Value.TotalHours >= 1
            ? Duration.Value.ToString(@"h\:mm\:ss")
            : Duration.Value.ToString(@"m\:ss");
}
