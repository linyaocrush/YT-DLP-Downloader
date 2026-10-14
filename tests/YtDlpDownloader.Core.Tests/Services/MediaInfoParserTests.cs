using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.Core.Tests.Services;

public class MediaInfoParserTests
{
    private readonly MediaInfoParser _parser = new();

    [Fact]
    public void Parse_FullPayload_MapsAllFields()
    {
        const string json = """
        {
          "id": "abc123",
          "title": "My Video",
          "webpage_url": "https://youtu.be/abc123",
          "uploader": "Uploader",
          "duration": 65,
          "thumbnail": "https://img/cover.jpg",
          "formats": [
            { "format_id": "137", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "width": 1920, "height": 1080, "fps": 30, "vbr": 4500 },
            { "format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a", "abr": 128 },
            { "format_id": "18", "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a", "width": 640, "height": 360, "fps": 30, "tbr": 800 }
          ]
        }
        """;

        var info = _parser.Parse(json);

        Assert.Equal("abc123", info.Id);
        Assert.Equal("My Video", info.Title);
        Assert.Equal("https://youtu.be/abc123", info.WebpageUrl);
        Assert.Equal("Uploader", info.Uploader);
        Assert.Equal(TimeSpan.FromSeconds(65), info.Duration);
        Assert.Equal("1:05", info.DurationText);
        Assert.Equal("https://img/cover.jpg", info.ThumbnailUrl);

        Assert.Equal(2, info.Videos.Count);
        var combined = Assert.Single(info.Videos, v => v.FormatId == "18");
        Assert.True(combined.HasAudio);
        Assert.Equal("mp4a", combined.AudioCodec);
        Assert.Equal("音视频合一", combined.Kind);

        var audioOnly = Assert.Single(info.Audios);
        Assert.Equal("140", audioOnly.FormatId);
        Assert.Equal(128, audioOnly.BitrateKbps);
    }

    [Fact]
    public void Parse_MissingTitle_FallsBackToId()
    {
        const string json = """
        { "id": "xyz", "formats": [ { "format_id": "1", "vcodec": "avc1", "height": 720 } ] }
        """;

        var info = _parser.Parse(json);

        Assert.Equal("xyz", info.Title);
        Assert.Equal(string.Empty, info.WebpageUrl);
        Assert.Equal(string.Empty, info.Uploader);
        Assert.Null(info.Duration);
    }

    [Fact]
    public void Parse_UsesChannelWhenUploaderMissing()
    {
        const string json = """
        { "id": "1", "channel": "The Channel", "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }
        """;

        Assert.Equal("The Channel", _parser.Parse(json).Uploader);
    }

    [Fact]
    public void Parse_StringNumbers_AreParsed()
    {
        const string json = """
        {
          "id": "1",
          "duration": "12.5",
          "formats": [ { "format_id": "1", "vcodec": "avc1", "width": "1920", "height": "1080", "fps": "29.97", "vbr": "4500" } ]
        }
        """;

        var info = _parser.Parse(json);

        Assert.Equal(TimeSpan.FromSeconds(12.5), info.Duration);
        var video = Assert.Single(info.Videos);
        Assert.Equal(1920, video.Width);
        Assert.Equal(1080, video.Height);
        Assert.Equal(29.97, video.Fps);
        Assert.Equal(4500, video.BitrateKbps);
    }

    [Fact]
    public void Parse_NoneAndEmptyCodecs_AreSkipped()
    {
        const string json = """
        {
          "id": "1",
          "formats": [
            { "format_id": "1", "vcodec": "none", "acodec": "none" },
            { "format_id": "2", "vcodec": "", "acodec": "" },
            { "format_id": "", "vcodec": "avc1" }
          ]
        }
        """;

        Assert.Throws<InvalidDataException>(() => _parser.Parse(json));
    }

    [Fact]
    public void Parse_NoFormatsArray_Throws()
    {
        Assert.Throws<InvalidDataException>(() => _parser.Parse("""{ "id": "1" }"""));
    }

    [Fact]
    public void Parse_NonObjectRoot_Throws()
    {
        Assert.Throws<InvalidDataException>(() => _parser.Parse("[]"));
    }

    [Fact]
    public void Parse_InvalidJson_Throws()
    {
        Assert.ThrowsAny<Exception>(() => _parser.Parse("not json"));
    }

    [Fact]
    public void Parse_SortsVideosByHeightThenBitrateThenFps()
    {
        const string json = """
        {
          "id": "1",
          "formats": [
            { "format_id": "a", "vcodec": "avc1", "height": 1080, "vbr": 3000, "fps": 30 },
            { "format_id": "b", "vcodec": "avc1", "height": 1080, "vbr": 5000, "fps": 30 },
            { "format_id": "c", "vcodec": "avc1", "height": 720,  "vbr": 9000, "fps": 60 },
            { "format_id": "d", "vcodec": "avc1", "height": 1080, "vbr": 5000, "fps": 60 }
          ]
        }
        """;

        var ids = _parser.Parse(json).Videos.Select(v => v.FormatId).ToArray();

        Assert.Equal(new[] { "d", "b", "a", "c" }, ids);
    }

    [Fact]
    public void Parse_SortsAudiosByBitrateDescending()
    {
        const string json = """
        {
          "id": "1",
          "formats": [
            { "format_id": "low",  "vcodec": "none", "acodec": "mp4a", "abr": 64 },
            { "format_id": "high", "vcodec": "none", "acodec": "opus", "abr": 256 }
          ]
        }
        """;

        var ids = _parser.Parse(json).Audios.Select(a => a.FormatId).ToArray();

        Assert.Equal(new[] { "high", "low" }, ids);
    }

    [Fact]
    public void Parse_ThumbnailsArray_PicksLargestArea()
    {
        const string json = """
        {
          "id": "1",
          "formats": [ { "format_id": "1", "vcodec": "avc1" } ],
          "thumbnails": [
            { "url": "small.jpg", "width": 100, "height": 100 },
            { "height": 500 },
            { "url": "large.jpg", "width": 1280, "height": 720 }
          ]
        }
        """;

        Assert.Equal("large.jpg", _parser.Parse(json).ThumbnailUrl);
    }

    [Fact]
    public void Parse_DirectThumbnailPreferredOverArray()
    {
        const string json = """
        {
          "id": "1",
          "thumbnail": "direct.jpg",
          "formats": [ { "format_id": "1", "vcodec": "avc1" } ],
          "thumbnails": [ { "url": "array.jpg", "width": 9999, "height": 9999 } ]
        }
        """;

        Assert.Equal("direct.jpg", _parser.Parse(json).ThumbnailUrl);
    }

    [Fact]
    public void Parse_VideoOnly_WhenNoAudio()
    {
        const string json = """
        { "id": "1", "formats": [ { "format_id": "1", "vcodec": "avc1", "height": 480 } ] }
        """;

        var info = _parser.Parse(json);

        Assert.Empty(info.Audios);
        Assert.Equal("纯视频", Assert.Single(info.Videos).Kind);
        Assert.Null(Assert.Single(info.Videos).AudioCodec);
    }

    [Fact]
    public void ParsePlaylist_PlaylistPayload_ReturnsEntries()
    {
        const string json = """
        {
          "_type": "playlist",
          "id": "PL1",
          "title": "My Playlist",
          "entries": [
            { "_type": "url", "id": "a", "title": "First", "url": "https://example.com/a", "webpage_url": "https://example.com/a" },
            { "_type": "url", "id": "b", "title": "Second", "url": "https://example.com/b" },
            { "_type": "url", "id": "c", "title": "No Url" }
          ]
        }
        """;

        var playlist = Assert.IsType<PlaylistInfo>(_parser.ParsePlaylist(json));

        Assert.Equal("PL1", playlist.Id);
        Assert.Equal("My Playlist", playlist.Title);
        Assert.Equal(2, playlist.Entries.Count);
        Assert.Equal("https://example.com/a", playlist.Entries[0].Url);
        Assert.Equal("First", playlist.Entries[0].Title);
        Assert.Equal("https://example.com/b", playlist.Entries[1].Url);
    }

    [Fact]
    public void ParsePlaylist_SingleVideo_ReturnsNull()
    {
        const string json = """
        { "_type": "video", "id": "1", "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }
        """;

        Assert.Null(_parser.ParsePlaylist(json));
    }

    [Fact]
    public void ParsePlaylist_EmptyEntries_ReturnsEmptyPlaylist()
    {
        const string json = """
        { "_type": "playlist", "id": "PL1", "title": "Empty", "entries": [] }
        """;

        var playlist = Assert.IsType<PlaylistInfo>(_parser.ParsePlaylist(json));

        Assert.Empty(playlist.Entries);
    }

    [Fact]
    public void Parse_IsLiveFlag_SetsIsLive()
    {
        const string json = """
        { "id": "1", "is_live": true, "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }
        """;

        var info = _parser.Parse(json);

        Assert.True(info.IsLive);
        Assert.False(info.IsUpcoming);
    }

    [Fact]
    public void Parse_LiveStatusIsLive_SetsIsLive()
    {
        const string json = """
        { "id": "1", "live_status": "is_live", "is_live": false, "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }
        """;

        Assert.True(_parser.Parse(json).IsLive);
    }

    [Fact]
    public void Parse_UpcomingStream_SetsIsUpcomingAndAllowsMissingFormats()
    {
        const string json = """
        { "id": "1", "title": "Premiere", "live_status": "is_upcoming", "is_live": false }
        """;

        var info = _parser.Parse(json);

        Assert.True(info.IsUpcoming);
        Assert.False(info.IsLive);
        Assert.Empty(info.Videos);
        Assert.Empty(info.Audios);
    }

    [Fact]
    public void Parse_FinishedLive_IsTreatedAsNormalVideo()
    {
        const string json = """
        { "id": "1", "live_status": "was_live", "was_live": true, "is_live": false, "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }
        """;

        var info = _parser.Parse(json);

        Assert.False(info.IsLive);
        Assert.False(info.IsUpcoming);
    }

    [Fact]
    public void Parse_NullIsLive_IsNotLive()
    {
        const string json = """
        { "id": "1", "is_live": null, "live_status": "not_live", "formats": [ { "format_id": "1", "vcodec": "avc1" } ] }
        """;

        Assert.False(_parser.Parse(json).IsLive);
    }
}
