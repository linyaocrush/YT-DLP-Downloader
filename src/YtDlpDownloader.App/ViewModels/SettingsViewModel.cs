using System.IO;
using YtDlpDownloader.Core.Mvvm;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.App.ViewModels;

public sealed class SettingsViewModel : ObservableObject
{
    private readonly ISettingsService _settings;
    private readonly IYtDlpPathService _paths;
    private readonly IYtDlpCli _cli;

    private string _ytDlpPath;
    private string _statusText = string.Empty;

    public SettingsViewModel(ISettingsService settings, IYtDlpPathService paths, IYtDlpCli cli)
    {
        _settings = settings;
        _paths = paths;
        _cli = cli;

        _ytDlpPath = settings.Settings.YtDlpPath ?? string.Empty;
        if (string.IsNullOrWhiteSpace(_ytDlpPath) || !File.Exists(_ytDlpPath))
        {
            var located = _paths.LocateInPath();
            if (located is not null)
            {
                _ytDlpPath = located;
                _settings.Settings.YtDlpPath = located;
                _settings.Save();
            }
        }

        BrowseCommand = new RelayCommand(Browse);
        RescanCommand = new RelayCommand(Rescan);

        _ = RefreshDetectionAsync();
    }

    public RelayCommand BrowseCommand { get; }
    public RelayCommand RescanCommand { get; }

    public string YtDlpPath
    {
        get => _ytDlpPath;
        set
        {
            var newValue = value ?? string.Empty;
            if (!SetProperty(ref _ytDlpPath, newValue))
                return;

            _settings.Settings.YtDlpPath = newValue.Trim();
            _settings.Save();
            _ = RefreshDetectionAsync();
        }
    }

    public string StatusText
    {
        get => _statusText;
        private set => SetProperty(ref _statusText, value);
    }

    private void Browse()
    {
        var dialog = new Microsoft.Win32.OpenFileDialog
        {
            Title = "选择 yt-dlp 可执行文件",
            Filter = "yt-dlp 可执行文件 (*.exe)|*.exe|所有文件 (*.*)|*.*",
            CheckFileExists = true,
        };

        var owner = System.Windows.Application.Current?.MainWindow;
        bool? result = owner is not null ? dialog.ShowDialog(owner) : dialog.ShowDialog();
        if (result == true)
            YtDlpPath = dialog.FileName;
    }

    private void Rescan()
    {
        // Re-run the PATH search; RefreshDetectionAsync also updates the text when a hit is found.
        _ = RefreshDetectionAsync();
    }

    private async Task RefreshDetectionAsync()
    {
        StatusText = "正在检测 yt-dlp…";

        try
        {
            string? resolved = _paths.Resolve();
            if (resolved is null)
            {
                StatusText = string.IsNullOrWhiteSpace(YtDlpPath)
                    ? "未在 PATH 中找到 yt-dlp。请点击“浏览…”选择 yt-dlp.exe，或直接输入完整路径。"
                    : "未检测到可用的 yt-dlp，请检查路径是否正确。";
                return;
            }

            // Reflect a PATH-found executable back into the field when nothing valid was configured.
            if (!string.Equals(resolved, YtDlpPath, StringComparison.OrdinalIgnoreCase))
            {
                _ytDlpPath = resolved;
                OnPropertyChanged(nameof(YtDlpPath));
            }

            var version = await _cli.GetVersionAsync();
            StatusText = version is null
                ? $"无法运行该程序，请确认它是有效的 yt-dlp：{resolved}"
                : $"已就绪 — yt-dlp {version}";
        }
        catch (OperationCanceledException)
        {
            // Ignored; only relevant if a cancellation token is added later.
        }
        catch (Exception ex)
        {
            StatusText = $"检测失败：{ex.Message}";
        }
    }
}
