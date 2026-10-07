using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.Core.Tests.Models;

public class YtDlpFormatTests
{
    [Fact]
    public void BuildExpression_DefaultBest_UsesAutoExpression()
        => Assert.Equal("bv*+ba/b",
            YtDlpFormat.BuildExpression(DownloadKind.Default, ResolutionPreference.Best));

    [Fact]
    public void BuildExpression_VideoOnlyBest_IsBv()
        => Assert.Equal("bv",
            YtDlpFormat.BuildExpression(DownloadKind.VideoOnly, ResolutionPreference.Best));

    [Fact]
    public void BuildExpression_AudioOnly_IgnoresResolution()
        => Assert.Equal("ba",
            YtDlpFormat.BuildExpression(DownloadKind.AudioOnly, ResolutionPreference.UpTo1080P));

    [Theory]
    [InlineData(ResolutionPreference.Above4K, "[height>2160]")]
    [InlineData(ResolutionPreference.UpTo4K, "[height<=2160]")]
    [InlineData(ResolutionPreference.UpTo2K, "[height<=1440]")]
    [InlineData(ResolutionPreference.UpTo1080P, "[height<=1080]")]
    [InlineData(ResolutionPreference.UpTo720P, "[height<=720]")]
    [InlineData(ResolutionPreference.UpTo360P, "[height<=360]")]
    public void BuildExpression_VideoOnly_AppliesResolutionFilter(
        ResolutionPreference preference, string expectedFilter)
        => Assert.Equal($"bv{expectedFilter}",
            YtDlpFormat.BuildExpression(DownloadKind.VideoOnly, preference));

    [Fact]
    public void BuildExpression_DefaultWithFilter_AppliesFilterToBothStreams()
        => Assert.Equal("bv*[height<=720]+ba/b[height<=720]",
            YtDlpFormat.BuildExpression(DownloadKind.Default, ResolutionPreference.UpTo720P));
}
