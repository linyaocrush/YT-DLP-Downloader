using YtDlpDownloader.App.ViewModels;
using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.App.Tests.ViewModels;

public class QueueItemViewModelTests
{
    private static VideoSource Video(
        string id = "137", bool hasAudio = false, string? audioCodec = null,
        int? width = 1920, int? height = 1080)
        => new(id, "mp4", "avc1", hasAudio, audioCodec, width, height, 30, 4500);

    private static AudioSource Audio(string id = "140")
        => new(id, "m4a", "mp4a", 128);

    [Fact]
    public void Constructor_InitializesUrlAsTitleAndPendingState()
    {
        var item = new QueueItemViewModel("https://example.com/v");

        Assert.Equal("https://example.com/v", item.Url);
        Assert.Equal("https://example.com/v", item.Title);
        Assert.True(item.IsPending);
        Assert.Equal("等待解析", item.StatusText);
        Assert.False(item.HasThumbnail);
    }

    [Fact]
    public void MarkParsing_SetsParsingState()
    {
        var item = new QueueItemViewModel("url");

        item.MarkParsing();

        Assert.True(item.IsParsing);
        Assert.True(item.ShowProgress);
        Assert.Equal("正在解析…", item.StatusText);
    }

    [Fact]
    public void ApplyInfo_SetsMetadataAndSources()
    {
        var item = new QueueItemViewModel("url");
        item.MarkFailed("previous error");

        item.ApplyInfo("New Title", "thumb.jpg", null, new[] { Video() }, new[] { Audio() });

        Assert.Equal("New Title", item.Title);
        Assert.Equal("thumb.jpg", item.ThumbnailUrl);
        Assert.True(item.IsParsed);
        Assert.Null(item.Error);
        Assert.Single(item.VideoSources);
        Assert.Single(item.AudioSources);
        Assert.False(item.HasThumbnail);
    }

    [Fact]
    public void ApplyInfo_BlankTitle_KeepsExistingTitle()
    {
        var item = new QueueItemViewModel("url");

        item.ApplyInfo("   ", null, null, Array.Empty<VideoSource>(), Array.Empty<AudioSource>());

        Assert.Equal("url", item.Title);
    }

    [Fact]
    public void MarkDownloading_ResetsProgressAndError()
    {
        var item = new QueueItemViewModel("url");
        item.ApplyInfo("t", null, null, Array.Empty<VideoSource>(), Array.Empty<AudioSource>());
        item.ReportProgress(42);

        item.MarkDownloading();

        Assert.True(item.IsDownloading);
        Assert.Equal(0, item.Progress);
        Assert.True(item.ShowProgress);
    }

    [Fact]
    public void ReportProgress_UpdatesPercentAndStatusText()
    {
        var item = new QueueItemViewModel("url");
        item.MarkDownloading();

        item.ReportProgress(37.5);

        Assert.Equal(37.5, item.Progress);
        Assert.Equal("38%", item.ProgressText);
        Assert.Equal("下载中 38%", item.StatusText);
    }

    [Fact]
    public void MarkCompleted_SetsOutputPathAndFullProgress()
    {
        var item = new QueueItemViewModel("url");

        item.MarkCompleted("C:/out.mp4");

        Assert.True(item.IsCompleted);
        Assert.Equal("C:/out.mp4", item.OutputPath);
        Assert.Equal(100, item.Progress);
        Assert.Equal("已完成", item.StatusText);
    }

    [Fact]
    public void MarkFailed_SetsErrorAndStatusText()
    {
        var item = new QueueItemViewModel("url");

        item.MarkFailed("network down");

        Assert.True(item.IsFailed);
        Assert.True(item.CanRetry);
        Assert.Equal("失败：network down", item.StatusText);
    }

    [Fact]
    public void MarkFailed_NullError_UsesGenericStatusText()
    {
        var item = new QueueItemViewModel("url");

        item.MarkFailed(null);

        Assert.Equal("失败", item.StatusText);
    }

    [Fact]
    public void MarkCancelled_AllowsRetry()
    {
        var item = new QueueItemViewModel("url");

        item.MarkCancelled();

        Assert.True(item.IsCancelled);
        Assert.True(item.CanRetry);
        Assert.Equal("已取消", item.StatusText);
    }

    [Fact]
    public void Reset_ReturnsToPendingAndClearsTransientState()
    {
        var item = new QueueItemViewModel("url");
        item.MarkFailed("err");
        item.MarkCompleted("path");

        item.Reset();

        Assert.True(item.IsPending);
        Assert.Null(item.Error);
        Assert.Null(item.OutputPath);
        Assert.Equal(0, item.Progress);
    }

    [Fact]
    public void HasValidManualSelection_MatrixByDownloadKind()
    {
        var item = new QueueItemViewModel("url");
        Assert.False(item.HasValidManualSelection(DownloadKind.Default));
        Assert.False(item.HasValidManualSelection(DownloadKind.VideoOnly));
        Assert.False(item.HasValidManualSelection(DownloadKind.AudioOnly));

        item.SelectedVideo = Video(hasAudio: true, audioCodec: "mp4a");
        Assert.True(item.HasValidManualSelection(DownloadKind.Default));
        Assert.True(item.HasValidManualSelection(DownloadKind.VideoOnly));
        Assert.False(item.HasValidManualSelection(DownloadKind.AudioOnly));

        item.SelectedVideo = Video(hasAudio: false);
        Assert.False(item.HasValidManualSelection(DownloadKind.Default));
        Assert.True(item.HasValidManualSelection(DownloadKind.VideoOnly));

        item.SelectedAudio = Audio();
        Assert.True(item.HasValidManualSelection(DownloadKind.Default));
        Assert.True(item.HasValidManualSelection(DownloadKind.AudioOnly));
    }

    [Fact]
    public void SelectionInfo_DescribesChosenSources()
    {
        var item = new QueueItemViewModel("url")
        {
            SelectedVideo = Video(),
            SelectedAudio = Audio(),
        };

        Assert.True(item.HasSelection);
        Assert.Contains("视频：1920x1080", item.SelectionInfo);
        Assert.Contains("avc1", item.SelectionInfo);
        Assert.Contains("音频：128 kbps", item.SelectionInfo);
    }

    [Fact]
    public void SelectionInfo_NoSelection_IsEmpty()
    {
        var item = new QueueItemViewModel("url");

        Assert.False(item.HasSelection);
        Assert.Equal(string.Empty, item.SelectionInfo);
    }

    [Fact]
    public void SelectedVideo_RaisesDependentPropertyChanged()
    {
        var item = new QueueItemViewModel("url");
        var raised = new List<string?>();
        item.PropertyChanged += (_, e) => raised.Add(e.PropertyName);

        item.SelectedVideo = Video();

        Assert.Contains(nameof(QueueItemViewModel.SelectionInfo), raised);
        Assert.Contains(nameof(QueueItemViewModel.HasSelection), raised);
    }

    [Fact]
    public void StateChange_RaisesStateDependentPropertyChanged()
    {
        var item = new QueueItemViewModel("url");
        var raised = new List<string?>();
        item.PropertyChanged += (_, e) => raised.Add(e.PropertyName);

        item.MarkCompleted("p");

        Assert.Contains(nameof(QueueItemViewModel.IsCompleted), raised);
        Assert.Contains(nameof(QueueItemViewModel.StatusText), raised);
        Assert.Contains(nameof(QueueItemViewModel.CanRetry), raised);
    }
}
