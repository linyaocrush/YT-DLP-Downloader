using System.Globalization;
using System.Text.Json;
using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.Core.Services;

public interface IMediaInfoParser
{
    MediaInfo Parse(string json);

    /// <summary>
    /// Returns a <see cref="PlaylistInfo"/> when <paramref name="json"/> describes a playlist,
    /// otherwise <c>null</c> so the caller can fall back to a single-video parse.
    /// </summary>
    PlaylistInfo? ParsePlaylist(string json);
}

/// <summary>
/// Maps the output of <c>yt-dlp --dump-single-json</c> onto <see cref="MediaInfo"/>.
/// Tolerant of the many shapes yt-dlp emits: numeric fields may be JSON numbers,
/// strings, null or absent; codec fields use "none" / "" to signal absence.
/// </summary>
public sealed class MediaInfoParser : IMediaInfoParser
{
    public MediaInfo Parse(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
            throw new InvalidDataException("yt-dlp 返回的数据不是有效的 JSON 对象");

        var id = Str(root, "id") ?? "-";
        var title = Str(root, "title") ?? id;
        var webpage = Str(root, "webpage_url") ?? string.Empty;
        var uploader = Str(root, "uploader") ?? Str(root, "channel") ?? string.Empty;

        // Live detection: "is_live" is the authoritative flag; "live_status" is a fallback and
        // tells apart a running stream (is_live), a scheduled one (is_upcoming) and a finished
        // one (was_live/post_live). The latter are ordinary videos and must not enter live mode.
        var liveStatus = Str(root, "live_status");
        var isLive = Bool(root, "is_live") == true
            || string.Equals(liveStatus, "is_live", StringComparison.OrdinalIgnoreCase);
        var isUpcoming = string.Equals(liveStatus, "is_upcoming", StringComparison.OrdinalIgnoreCase)
            || Bool(root, "is_upcoming") == true;

        TimeSpan? duration = null;
        var durationSeconds = Num(root, "duration");
        if (durationSeconds is not null)
            duration = TimeSpan.FromSeconds(durationSeconds.Value);

        var videos = new List<VideoSource>();
        var audios = new List<AudioSource>();

        if (root.TryGetProperty("formats", out var formats) && formats.ValueKind == JsonValueKind.Array)
        {
            foreach (var format in formats.EnumerateArray())
            {
                if (format.ValueKind != JsonValueKind.Object)
                    continue;

                var formatId = Str(format, "format_id");
                if (string.IsNullOrEmpty(formatId))
                    continue;

                var hasVideo = IsPresent(format, "vcodec");
                var hasAudio = IsPresent(format, "acodec");
                if (!hasVideo && !hasAudio)
                    continue;

                var container = Str(format, "ext") ?? "-";

                if (hasVideo)
                {
                    var vcodec = Str(format, "vcodec") ?? "-";
                    var acodec = hasAudio ? Str(format, "acodec") : null;
                    videos.Add(new VideoSource(
                        formatId,
                        container,
                        vcodec,
                        hasAudio,
                        acodec,
                        (int?)Num(format, "width"),
                        (int?)Num(format, "height"),
                        Num(format, "fps"),
                        Num(format, "vbr") ?? Num(format, "tbr") ?? Num(format, "abr")));
                }
                else if (hasAudio)
                {
                    audios.Add(new AudioSource(
                        formatId,
                        container,
                        Str(format, "acodec") ?? "-",
                        Num(format, "abr") ?? Num(format, "tbr")));
                }
            }
        }

        // Prefer the most attractive source first (highest height, then bitrate, then fps).
        videos.Sort((a, b) =>
        {
            var byHeight = b.Height.GetValueOrDefault().CompareTo(a.Height.GetValueOrDefault());
            if (byHeight != 0)
                return byHeight;

            var byBitrate = b.BitrateKbps.GetValueOrDefault().CompareTo(a.BitrateKbps.GetValueOrDefault());
            return byBitrate != 0 ? byBitrate : b.Fps.GetValueOrDefault().CompareTo(a.Fps.GetValueOrDefault());
        });

        audios.Sort((a, b) =>
            b.BitrateKbps.GetValueOrDefault().CompareTo(a.BitrateKbps.GetValueOrDefault()));

        // An upcoming stream exposes no formats yet; keep it parseable so the UI can prompt instead
        // of reporting a generic parse failure.
        if (videos.Count == 0 && audios.Count == 0 && !isLive && !isUpcoming)
            throw new InvalidDataException("未解析到任何可用的视频/音频格式");

        return new MediaInfo(id, title, webpage, duration, uploader, BestThumbnail(root), videos, audios)
        {
            IsLive = isLive,
            IsUpcoming = isUpcoming,
        };
    }

    public PlaylistInfo? ParsePlaylist(string json)
    {
        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;
        if (root.ValueKind != JsonValueKind.Object)
            return null;

        // yt-dlp marks playlist containers with "_type": "playlist"; a single video never has this.
        if (!string.Equals(Str(root, "_type"), "playlist", StringComparison.OrdinalIgnoreCase))
            return null;

        if (!root.TryGetProperty("entries", out var entries) || entries.ValueKind != JsonValueKind.Array)
            return null;

        var items = new List<PlaylistEntry>();
        foreach (var entry in entries.EnumerateArray())
        {
            if (entry.ValueKind != JsonValueKind.Object)
                continue;

            // Flat listings expose the canonical page URL as "webpage_url" and often as "url" too.
            var url = Str(entry, "webpage_url") ?? Str(entry, "url");
            if (string.IsNullOrWhiteSpace(url))
                continue;

            items.Add(new PlaylistEntry(url, Str(entry, "title") ?? url));
        }

        return new PlaylistInfo(Str(root, "id") ?? "-", Str(root, "title") ?? "播放列表", items);
    }

    /// <summary>Prefers the top-level "thumbnail" URL, otherwise the largest entry of "thumbnails".</summary>
    private static string? BestThumbnail(JsonElement root)
    {
        var direct = Str(root, "thumbnail");
        if (!string.IsNullOrEmpty(direct))
            return direct;

        if (!root.TryGetProperty("thumbnails", out var thumbnails) || thumbnails.ValueKind != JsonValueKind.Array)
            return null;

        string? best = null;
        var bestArea = -1d;
        foreach (var thumbnail in thumbnails.EnumerateArray())
        {
            var url = Str(thumbnail, "url");
            if (string.IsNullOrEmpty(url))
                continue;

            var area = (Num(thumbnail, "width") ?? 0) * (Num(thumbnail, "height") ?? 0);
            if (area >= bestArea)
            {
                bestArea = area;
                best = url;
            }
        }

        return best;
    }

    private static bool IsPresent(JsonElement obj, string name)
    {
        if (obj.ValueKind != JsonValueKind.Object || !obj.TryGetProperty(name, out var element))
            return false;

        if (element.ValueKind != JsonValueKind.String)
            return false;

        var value = element.GetString();
        return !string.IsNullOrEmpty(value)
            && !string.Equals(value, "none", StringComparison.OrdinalIgnoreCase);
    }

    private static string? Str(JsonElement obj, string name)
    {
        if (obj.ValueKind != JsonValueKind.Object)
            return null;
        if (!obj.TryGetProperty(name, out var element) || element.ValueKind != JsonValueKind.String)
            return null;

        var value = element.GetString();
        return string.IsNullOrEmpty(value) ? null : value;
    }

    private static double? Num(JsonElement obj, string name)
    {
        if (obj.ValueKind != JsonValueKind.Object || !obj.TryGetProperty(name, out var element))
            return null;

        return element.ValueKind switch
        {
            JsonValueKind.Number => element.TryGetDouble(out var number) ? number : null,
            JsonValueKind.String => double.TryParse(element.GetString(), NumberStyles.Float,
                CultureInfo.InvariantCulture, out var parsed) ? parsed : null,
            _ => null,
        };
    }

    private static bool? Bool(JsonElement obj, string name)
    {
        if (obj.ValueKind != JsonValueKind.Object || !obj.TryGetProperty(name, out var element))
            return null;

        return element.ValueKind switch
        {
            JsonValueKind.True => true,
            JsonValueKind.False => false,
            JsonValueKind.String => bool.TryParse(element.GetString(), out var parsed) ? parsed : null,
            _ => null,
        };
    }
}
