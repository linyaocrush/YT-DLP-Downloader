using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.Core.Tests.Services;

public class SettingsServiceTests
{
    [Fact]
    public void Constructor_NoFile_UsesDefaults()
    {
        using var temp = new TempDirectory();

        var service = new SettingsService(temp.Path);

        Assert.Equal(4, service.Settings.DownloadThreads);
        Assert.Equal("%(title)s", service.Settings.OutputTemplate);
        Assert.Equal(AppTheme.System, service.Settings.Theme);
        Assert.Equal(OutputNameMode.Template, service.Settings.OutputNameMode);
        Assert.Equal(string.Empty, service.Settings.YtDlpPath);
    }

    [Fact]
    public void Save_ThenReload_RoundTripsPersistedValues()
    {
        using var temp = new TempDirectory();

        var first = new SettingsService(temp.Path);
        first.Settings.YtDlpPath = "C:/yt-dlp.exe";
        first.Settings.DownloadDirectory = "C:/downloads";
        first.Settings.Theme = AppTheme.Dark;
        first.Settings.DownloadKind = DownloadKind.AudioOnly;
        first.Settings.Resolution = ResolutionPreference.UpTo1080P;
        first.Settings.ProxyEnabled = true;
        first.Settings.ProxyListMode = ProxyListMode.Blacklist;
        first.Settings.ProxySites.Add("youtube.com");
        first.Save();

        var reloaded = new SettingsService(temp.Path);

        Assert.Equal("C:/yt-dlp.exe", reloaded.Settings.YtDlpPath);
        Assert.Equal("C:/downloads", reloaded.Settings.DownloadDirectory);
        Assert.Equal(AppTheme.Dark, reloaded.Settings.Theme);
        Assert.Equal(DownloadKind.AudioOnly, reloaded.Settings.DownloadKind);
        Assert.Equal(ResolutionPreference.UpTo1080P, reloaded.Settings.Resolution);
        Assert.True(reloaded.Settings.ProxyEnabled);
        Assert.Equal(ProxyListMode.Blacklist, reloaded.Settings.ProxyListMode);
        Assert.Equal(new[] { "youtube.com" }, reloaded.Settings.ProxySites);
    }

    [Fact]
    public void Save_WritesEnumsAsStrings()
    {
        using var temp = new TempDirectory();
        var service = new SettingsService(temp.Path);
        service.Settings.Theme = AppTheme.Dark;
        service.Settings.DownloadKind = DownloadKind.AudioOnly;

        service.Save();

        var text = File.ReadAllText(Path.Combine(temp.Path, "settings.json"));
        Assert.Contains("\"Dark\"", text);
        Assert.Contains("\"AudioOnly\"", text);
    }

    [Fact]
    public void Constructor_CorruptedFile_FallsBackToDefaults()
    {
        using var temp = new TempDirectory();
        File.WriteAllText(Path.Combine(temp.Path, "settings.json"), "{ this is not valid json");

        var service = new SettingsService(temp.Path);

        Assert.Equal(4, service.Settings.DownloadThreads);
        Assert.Equal(AppTheme.System, service.Settings.Theme);
    }

    [Fact]
    public void Save_RaisesChangedEvent()
    {
        using var temp = new TempDirectory();
        var service = new SettingsService(temp.Path);
        var raised = 0;
        service.Changed += (_, _) => raised++;

        service.Save();
        service.Save();

        Assert.Equal(2, raised);
    }
}
