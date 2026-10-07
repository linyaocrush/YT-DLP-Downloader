using Moq;
using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.Core.Tests.Services;

/// <summary>
/// Groups tests that temporarily mutate process-wide environment variables so they never run
/// concurrently with each other.
/// </summary>
[CollectionDefinition("EnvironmentVariables", DisableParallelization = true)]
public sealed class EnvironmentVariablesCollection
{
}

[Collection("EnvironmentVariables")]
public class PathServiceTests
{
    private static ISettingsService SettingsService(AppSettings settings)
    {
        var mock = new Mock<ISettingsService>();
        mock.SetupGet(s => s.Settings).Returns(settings);
        return mock.Object;
    }

    private static string? SetPath(string value, Func<string?> action)
    {
        var original = Environment.GetEnvironmentVariable("PATH");
        try
        {
            Environment.SetEnvironmentVariable("PATH", value);
            return action();
        }
        finally
        {
            Environment.SetEnvironmentVariable("PATH", original);
        }
    }

    private static string SystemExecutable(string name)
        => Path.Combine(Environment.SystemDirectory, name);

    [Fact]
    public void Ffmpeg_GetConfiguredPath_ExistingFile_ReturnsPath()
    {
        using var temp = new TempDirectory();
        var exe = temp.CreateFile("ffmpeg.exe");
        var service = new FfmpegPathService(SettingsService(new AppSettings { FfmpegPath = exe }));

        Assert.Equal(exe, service.GetConfiguredPath());
    }

    [Fact]
    public void Ffmpeg_GetConfiguredPath_MissingFile_ReturnsNull()
    {
        using var temp = new TempDirectory();
        var service = new FfmpegPathService(
            SettingsService(new AppSettings { FfmpegPath = Path.Combine(temp.Path, "nope.exe") }));

        Assert.Null(service.GetConfiguredPath());
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    public void Ffmpeg_GetConfiguredPath_Blank_ReturnsNull(string configured)
    {
        var service = new FfmpegPathService(SettingsService(new AppSettings { FfmpegPath = configured }));

        Assert.Null(service.GetConfiguredPath());
    }

    [Fact]
    public void Ffmpeg_GetExplicitPathForYtDlp_NoPathHit_ReturnsConfiguredPath()
    {
        using var temp = new TempDirectory();
        var empty = Path.Combine(temp.Path, "empty");
        Directory.CreateDirectory(empty);
        var ffmpeg = temp.CreateFile("manual/ffmpeg.exe");
        var service = new FfmpegPathService(SettingsService(new AppSettings { FfmpegPath = ffmpeg }));

        var result = SetPath(empty, service.GetExplicitPathForYtDlp);

        Assert.Equal(ffmpeg, result);
    }

    [Fact]
    public void Ffmpeg_LocateInPath_NoPathVariable_ReturnsNull()
    {
        var service = new FfmpegPathService(SettingsService(new AppSettings()));

        Assert.Null(SetPath(string.Empty, service.LocateInPath));
    }

    [Fact]
    public void YtDlp_Resolve_ConfiguredExisting_ReturnsWithoutSave()
    {
        using var temp = new TempDirectory();
        var exe = temp.CreateFile("yt-dlp.exe");
        var settings = new AppSettings { YtDlpPath = exe };
        var mock = new Mock<ISettingsService>();
        mock.SetupGet(s => s.Settings).Returns(settings);

        var result = new YtDlpPathService(mock.Object).Resolve();

        Assert.Equal(exe, result);
        mock.Verify(s => s.Save(), Times.Never);
    }

    [Fact]
    public void YtDlp_Resolve_FoundOnPath_PersistsAndReturns()
    {
        using var temp = new TempDirectory();
        var exe = temp.CreateFile("yt-dlp.exe");
        var settings = new AppSettings();
        var mock = new Mock<ISettingsService>();
        mock.SetupGet(s => s.Settings).Returns(settings);
        var service = new YtDlpPathService(mock.Object);

        var result = SetPath(temp.Path, service.Resolve);

        Assert.Equal(exe, result);
        Assert.Equal(exe, settings.YtDlpPath);
        mock.Verify(s => s.Save(), Times.Once);
    }

    [Fact]
    public void YtDlp_Resolve_NothingConfiguredOrOnPath_ReturnsNull()
    {
        using var temp = new TempDirectory();
        var empty = Path.Combine(temp.Path, "empty");
        Directory.CreateDirectory(empty);
        var settings = new AppSettings { YtDlpPath = Path.Combine(temp.Path, "missing.exe") };
        var mock = new Mock<ISettingsService>();
        mock.SetupGet(s => s.Settings).Returns(settings);
        var service = new YtDlpPathService(mock.Object);

        var result = SetPath(empty, service.Resolve);

        Assert.Null(result);
        mock.Verify(s => s.Save(), Times.Never);
    }
}
