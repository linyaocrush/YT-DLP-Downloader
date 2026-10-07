using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.Core.Tests.Models;

public class MediaModelTests
{
    private static VideoSource Video(
        int? width = 1920, int? height = 1080, double? fps = 30,
        double? bitrate = 4500, bool hasAudio = false, string? audioCodec = null)
        => new("137", "mp4", "avc1", hasAudio, audioCodec, width, height, fps, bitrate);

    [Fact]
    public void VideoSource_Resolution_FormatsBothDimensions()
        => Assert.Equal("1920x1080", Video().Resolution);

    [Fact]
    public void VideoSource_Resolution_MissingDimension_IsDash()
        => Assert.Equal("-", Video(height: null).Resolution);

    [Fact]
    public void VideoSource_FpsText_NullIsDash()
        => Assert.Equal("-", Video(fps: null).FpsText);

    [Fact]
    public void VideoSource_FpsText_WholeNumber_NoDecimal()
        => Assert.Equal("30", Video(fps: 29.999).FpsText);

    [Fact]
    public void VideoSource_FpsText_Fractional_OneDecimal()
        => Assert.Equal("23.4", Video(fps: 23.4).FpsText);

    [Fact]
    public void VideoSource_FpsText_Fractional_RoundsToWholeWhenClose()
        => Assert.Equal("24", Video(fps: 23.976).FpsText);

    [Fact]
    public void VideoSource_BitrateText_NullIsDash()
        => Assert.Equal("-", Video(bitrate: null).BitrateText);

    [Fact]
    public void VideoSource_BitrateText_RoundsToWholeKbps()
        => Assert.Equal("4500 kbps", Video(bitrate: 4499.6).BitrateText);

    [Fact]
    public void VideoSource_Kind_DependsOnAudioPresence()
    {
        Assert.Equal("纯视频", Video(hasAudio: false).Kind);
        Assert.Equal("音视频合一", Video(hasAudio: true, audioCodec: "mp4a").Kind);
    }

    [Fact]
    public void AudioSource_BitrateText_FormatsAndHandlesNull()
    {
        Assert.Equal("128 kbps", new AudioSource("140", "m4a", "mp4a", 128.4).BitrateText);
        Assert.Equal("-", new AudioSource("140", "m4a", "mp4a", null).BitrateText);
    }

    private static MediaInfo Info(TimeSpan? duration)
        => new("id", "title", "url", duration, "up", null,
            Array.Empty<VideoSource>(), Array.Empty<AudioSource>());

    [Fact]
    public void MediaInfo_DurationText_NullIsDash()
        => Assert.Equal("-", Info(null).DurationText);

    [Fact]
    public void MediaInfo_DurationText_UnderOneHour_MinutesSeconds()
        => Assert.Equal("3:05", Info(TimeSpan.FromSeconds(185)).DurationText);

    [Fact]
    public void MediaInfo_DurationText_OneHourOrMore_HoursMinutesSeconds()
        => Assert.Equal("1:02:03", Info(new TimeSpan(1, 2, 3)).DurationText);
}
