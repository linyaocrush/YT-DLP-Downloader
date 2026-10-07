using Moq;
using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.Core.Tests.Services;

public class YtDlpCliTests
{
    private readonly Mock<IProcessRunner> _runner = new();
    private readonly Mock<IYtDlpPathService> _paths = new();
    private readonly Mock<IFfmpegPathService> _ffmpeg = new();
    private readonly Mock<IMediaInfoParser> _parser = new();

    private YtDlpCli Create() => new(_runner.Object, _paths.Object, _ffmpeg.Object, _parser.Object);

    private List<string> CaptureArguments(ProcessResult result)
    {
        var args = new List<string>();
        _runner
            .Setup(r => r.RunAsync(
                It.IsAny<string>(),
                It.IsAny<IReadOnlyList<string>>(),
                It.IsAny<Action<string>?>(),
                It.IsAny<Action<string>?>(),
                It.IsAny<CancellationToken>()))
            .Callback<string, IReadOnlyList<string>, Action<string>?, Action<string>?, CancellationToken>(
                (_, a, _, _, _) => args.AddRange(a))
            .ReturnsAsync(result);
        return args;
    }

    private List<string> CaptureArgumentsWithLines(ProcessResult result, IEnumerable<string> lines)
    {
        var args = new List<string>();
        _runner
            .Setup(r => r.RunAsync(
                It.IsAny<string>(),
                It.IsAny<IReadOnlyList<string>>(),
                It.IsAny<Action<string>?>(),
                It.IsAny<Action<string>?>(),
                It.IsAny<CancellationToken>()))
            .Callback<string, IReadOnlyList<string>, Action<string>?, Action<string>?, CancellationToken>(
                (_, a, onStdOut, _, _) =>
                {
                    args.AddRange(a);
                    foreach (var line in lines)
                        onStdOut?.Invoke(line);
                })
            .ReturnsAsync(result);
        return args;
    }

    private sealed class RecordingProgress : IProgress<DownloadUpdate>
    {
        public List<DownloadUpdate> Items { get; } = new();

        public void Report(DownloadUpdate value) => Items.Add(value);
    }

    [Fact]
    public void ResolveExecutable_DelegatesToPathService()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");

        Assert.Equal("C:/yt-dlp.exe", Create().ResolveExecutable());
    }

    [Fact]
    public async Task GetVersionAsync_NoExecutable_ReturnsNullWithoutRunning()
    {
        _paths.Setup(p => p.Resolve()).Returns((string?)null);

        Assert.Null(await Create().GetVersionAsync());
        _runner.Verify(r => r.RunAsync(
            It.IsAny<string>(), It.IsAny<IReadOnlyList<string>>(),
            It.IsAny<Action<string>?>(), It.IsAny<Action<string>?>(),
            It.IsAny<CancellationToken>()), Times.Never);
    }

    [Fact]
    public async Task GetVersionAsync_Success_ReturnsTrimmedVersion()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArguments(new ProcessResult(false, 0, "  2024.01.01\n", string.Empty));

        Assert.Equal("2024.01.01", await Create().GetVersionAsync());
    }

    [Fact]
    public async Task GetVersionAsync_NonZeroExit_ReturnsNull()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArguments(new ProcessResult(false, 1, "boom", string.Empty));

        Assert.Null(await Create().GetVersionAsync());
    }

    [Fact]
    public async Task GetMediaInfoAsync_NoExecutable_Throws()
    {
        _paths.Setup(p => p.Resolve()).Returns((string?)null);

        await Assert.ThrowsAsync<YtDlpException>(() => Create().GetMediaInfoAsync("url"));
    }

    [Fact]
    public async Task GetMediaInfoAsync_Success_PassesArgumentsAndParses()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        var args = CaptureArguments(new ProcessResult(false, 0, "{}", string.Empty));
        var expected = new MediaInfo("1", "t", "u", null, null, null,
            Array.Empty<VideoSource>(), Array.Empty<AudioSource>());
        _parser.Setup(p => p.Parse("{}")).Returns(expected);

        var info = await Create().GetMediaInfoAsync(
            "https://youtu.be/1", cookieFile: "c.txt", proxyUrl: "http://p:1");

        Assert.Same(expected, info);
        Assert.Contains("--dump-single-json", args);
        Assert.Contains("--skip-download", args);
        Assert.Equal("https://youtu.be/1", args[^1]);
        Assert.Contains("--cookies", args);
        Assert.Contains("c.txt", args);
        Assert.Contains("--proxy", args);
        Assert.Contains("http://p:1", args);
    }

    [Fact]
    public async Task GetMediaInfoAsync_Cancelled_ThrowsOperationCanceled()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArguments(new ProcessResult(true, 0, string.Empty, string.Empty));

        await Assert.ThrowsAsync<OperationCanceledException>(
            () => Create().GetMediaInfoAsync("url"));
    }

    [Fact]
    public async Task GetMediaInfoAsync_NonZeroExit_ThrowsWithLastMeaningfulError()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArguments(new ProcessResult(false, 1, "stdout tail", "line1\nline2"));

        var ex = await Assert.ThrowsAsync<YtDlpException>(
            () => Create().GetMediaInfoAsync("url"));

        Assert.Contains("line2", ex.Message);
        Assert.Contains("stdout tail", ex.Message);
    }

    [Fact]
    public async Task GetMediaInfoAsync_ParserFailure_WrappedInYtDlpException()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArguments(new ProcessResult(false, 0, "{}", string.Empty));
        _parser.Setup(p => p.Parse(It.IsAny<string>()))
            .Throws(new InvalidDataException("bad data"));

        var ex = await Assert.ThrowsAsync<YtDlpException>(
            () => Create().GetMediaInfoAsync("url"));

        Assert.Contains("解析视频信息失败", ex.Message);
        Assert.Contains("bad data", ex.Message);
    }

    [Fact]
    public async Task DownloadAsync_NoExecutable_ReturnsErrorOutcome()
    {
        _paths.Setup(p => p.Resolve()).Returns((string?)null);

        var outcome = await Create().DownloadAsync("url", "bv", "C:/out");

        Assert.False(outcome.Success);
        Assert.False(outcome.Cancelled);
        Assert.NotNull(outcome.Error);
    }

    [Fact]
    public async Task DownloadAsync_Success_BuildsExpectedArguments()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        _ffmpeg.Setup(f => f.GetExplicitPathForYtDlp()).Returns("C:/ffmpeg.exe");
        var output = Path.GetTempPath();
        var args = CaptureArguments(new ProcessResult(false, 0, string.Empty, string.Empty));

        var outcome = await Create().DownloadAsync(
            "https://youtu.be/1",
            "bv*[height<=720]+ba",
            output,
            cookieFile: "cookies.txt",
            proxyUrl: "socks5://p:1080",
            concurrentFragments: 3,
            outputTemplate: "%(id)s.%(ext)s",
            writeThumbnail: true);

        Assert.True(outcome.Success);
        Assert.Contains("-f", args);
        Assert.Contains("bv*[height<=720]+ba", args);
        Assert.Contains("--cookies", args);
        Assert.Contains("cookies.txt", args);
        Assert.Contains("--proxy", args);
        Assert.Contains("socks5://p:1080", args);
        Assert.Contains("--ffmpeg-location", args);
        Assert.Contains("C:/ffmpeg.exe", args);
        Assert.Contains("--concurrent-fragments", args);
        Assert.Contains("3", args);
        Assert.Contains("--write-thumbnail", args);
        Assert.Contains(Path.Combine(output, "%(id)s.%(ext)s"), args);
    }

    [Fact]
    public async Task DownloadAsync_DefaultTemplateAndSingleFragment()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        var output = Path.GetTempPath();
        var args = CaptureArguments(new ProcessResult(false, 0, string.Empty, string.Empty));

        await Create().DownloadAsync("url", "bv", output);

        Assert.DoesNotContain("--concurrent-fragments", args);
        Assert.DoesNotContain("--write-thumbnail", args);
        Assert.DoesNotContain("--cookies", args);
        Assert.DoesNotContain("--proxy", args);
        Assert.Contains(Path.Combine(output, "%(title)s.%(ext)s"), args);
    }

    [Fact]
    public async Task DownloadAsync_DestinationThenMerge_UsesMergedPath()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        var output = Path.GetTempPath();
        CaptureArgumentsWithLines(new ProcessResult(false, 0, string.Empty, string.Empty), new[]
        {
            "[download] Destination: C:/out/video.f137.mp4",
            "Merging formats into \"C:/out/final.mp4\"",
        });
        var progress = new RecordingProgress();

        var outcome = await Create().DownloadAsync("url", "bv", output, progress: progress);

        Assert.True(outcome.Success);
        Assert.Equal("C:/out/final.mp4", outcome.OutputPath);
    }

    [Fact]
    public async Task DownloadAsync_OnlyDestination_UsesDestination()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArgumentsWithLines(new ProcessResult(false, 0, string.Empty, string.Empty), new[]
        {
            "[download] Destination: C:/out/only.mp4",
        });
        var progress = new RecordingProgress();

        var outcome = await Create().DownloadAsync("url", "bv", Path.GetTempPath(), progress: progress);

        Assert.Equal("C:/out/only.mp4", outcome.OutputPath);
    }

    [Fact]
    public async Task DownloadAsync_ProgressLines_ReportPercentSpeedEta()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        _ffmpeg.Setup(f => f.GetExplicitPathForYtDlp()).Returns((string?)null);
        CaptureArgumentsWithLines(new ProcessResult(false, 0, string.Empty, string.Empty), new[]
        {
            "[download]  50.0% of 10.00MiB at 1.00MiB/s ETA 00:05",
            "[download]  150.0% of 10.00MiB at 2.00MiB/s ETA 00:01",
            "some plain status line",
        });
        var progress = new RecordingProgress();

        await Create().DownloadAsync("url", "bv", Path.GetTempPath(), progress: progress);

        Assert.Equal(3, progress.Items.Count);
        Assert.Equal(50, progress.Items[0].Percent);
        Assert.Equal("1.00MiB/s", progress.Items[0].Speed);
        Assert.Equal("00:05", progress.Items[0].Eta);
        Assert.Equal(100, progress.Items[1].Percent);
        Assert.Equal("some plain status line", progress.Items[2].Line);
    }

    [Fact]
    public async Task DownloadAsync_CancelledResult_ReturnsCancelledOutcome()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArguments(new ProcessResult(true, 1, string.Empty, string.Empty));

        var outcome = await Create().DownloadAsync("url", "bv", Path.GetTempPath());

        Assert.True(outcome.Cancelled);
        Assert.False(outcome.Success);
    }

    [Fact]
    public async Task DownloadAsync_RunnerThrowsOperationCanceled_ReturnsCancelledOutcome()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        _runner
            .Setup(r => r.RunAsync(
                It.IsAny<string>(),
                It.IsAny<IReadOnlyList<string>>(),
                It.IsAny<Action<string>?>(),
                It.IsAny<Action<string>?>(),
                It.IsAny<CancellationToken>()))
            .ThrowsAsync(new OperationCanceledException());

        var outcome = await Create().DownloadAsync("url", "bv", Path.GetTempPath());

        Assert.True(outcome.Cancelled);
        Assert.Null(outcome.Error);
    }

    [Fact]
    public async Task DownloadAsync_NonZeroExit_ReturnsErrorOutcome()
    {
        _paths.Setup(p => p.Resolve()).Returns("C:/yt-dlp.exe");
        CaptureArguments(new ProcessResult(false, 2, "out", "ERROR: failed badly"));

        var outcome = await Create().DownloadAsync("url", "bv", Path.GetTempPath());

        Assert.False(outcome.Success);
        Assert.False(outcome.Cancelled);
        Assert.Contains("failed badly", outcome.Error);
    }
}
