using System.Windows;
using YtDlpDownloader.App.Services;
using YtDlpDownloader.App.Theming;
using YtDlpDownloader.App.ViewModels;
using YtDlpDownloader.App.Views;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.App;

public partial class App : Application
{
    private ISettingsService? _settings;
    private IYtDlpCli? _cli;
    private DownloadViewModel? _download;
    private TaskbarProgressBinder? _taskbarBinder;
    private DownloadNotifier? _notifier;

    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);

        // ---- Composition root (plain DI, no external packages) ----
        var settingsService = new SettingsService(AppContext.BaseDirectory);
        ThemeManager.Apply(settingsService.Settings.Theme);

        var pathService = new YtDlpPathService(settingsService);
        var ffmpegPathService = new FfmpegPathService(settingsService);
        var runner = new ProcessRunner();
        var parser = new MediaInfoParser();
        var cli = new YtDlpCli(runner, pathService, ffmpegPathService, parser);

        var settingsViewModel = new SettingsViewModel(settingsService, pathService, ffmpegPathService, cli);
        var downloadViewModel = new DownloadViewModel(settingsService, cli);

        settingsService.Changed += (_, _) => downloadViewModel.RefreshYtDlpStatus();
        settingsService.Changed += (_, _) => ThemeManager.Apply(settingsService.Settings.Theme);

        _settings = settingsService;
        _cli = cli;
        _download = downloadViewModel;

        var mainWindow = new MainWindow
        {
            DataContext = new MainViewModel(downloadViewModel, settingsViewModel),
        };
        MainWindow = mainWindow;
        mainWindow.Show();
        ThemeManager.ApplyWindowChrome(mainWindow);

        _taskbarBinder = new TaskbarProgressBinder(mainWindow, downloadViewModel);
        _notifier = new DownloadNotifier(mainWindow, downloadViewModel);
    }

    protected override void OnExit(ExitEventArgs e)
    {
        _notifier?.Dispose();
        _taskbarBinder?.Dispose();
        _download?.Cleanup();
        base.OnExit(e);
    }
}
