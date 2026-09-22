using System.Windows;
using YtDlpDownloader.App.ViewModels;
using YtDlpDownloader.App.Views;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.App;

public partial class App : Application
{
    private ISettingsService? _settings;
    private IYtDlpCli? _cli;
    private DownloadViewModel? _download;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // ---- Composition root (plain DI, no external packages) ----
        var settingsService = new SettingsService(AppContext.BaseDirectory);
        var pathService = new YtDlpPathService(settingsService);
        var ffmpegPathService = new FfmpegPathService(settingsService);
        var runner = new ProcessRunner();
        var parser = new MediaInfoParser();
        var cli = new YtDlpCli(runner, pathService, ffmpegPathService, parser);

        var settingsViewModel = new SettingsViewModel(settingsService, pathService, ffmpegPathService, cli);
        var downloadViewModel = new DownloadViewModel(settingsService, cli);

        settingsService.Changed += (_, _) => downloadViewModel.RefreshYtDlpStatus();

        _settings = settingsService;
        _cli = cli;
        _download = downloadViewModel;

        var mainWindow = new MainWindow
        {
            DataContext = new MainViewModel(downloadViewModel, settingsViewModel),
        };
        MainWindow = mainWindow;
        mainWindow.Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _download?.Cleanup();
        base.OnExit(e);
    }
}
