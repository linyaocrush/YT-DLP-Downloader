using System.Collections.ObjectModel;
using System.IO;
using System.Linq;
using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Mvvm;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.App.ViewModels;

/// <summary>A selectable proxy protocol and its display prefix.</summary>
public sealed record ProxyProtocolOption(ProxyProtocol Value, string Label);

public sealed class SettingsViewModel : ObservableObject
{
    /// <summary>Protocol choices offered in the proxy card.</summary>
    public static readonly IReadOnlyList<ProxyProtocolOption> ProxyProtocolCatalog = new[]
    {
        new ProxyProtocolOption(ProxyProtocol.Http, "http"),
        new ProxyProtocolOption(ProxyProtocol.Https, "https"),
        new ProxyProtocolOption(ProxyProtocol.Socks5, "socks5"),
    };
    private readonly ISettingsService _settings;
    private readonly IYtDlpPathService _paths;
    private readonly IFfmpegPathService _ffmpegPaths;
    private readonly IYtDlpCli _cli;

    private string _ytDlpPath;
    private string _statusText = string.Empty;
    private string _ffmpegPath;
    private string _ffmpegStatus = string.Empty;
    private bool _showFfmpegManualControls;
    private string _cookieFolder;
    private string _cookieFolderStatus = string.Empty;

    private bool _proxyEnabled;
    private ProxyProtocolOption _selectedProxyProtocol;
    private string _proxyHost;
    private string _proxyPort;
    private ProxyListMode _proxyListMode;
    private string _newProxySite = string.Empty;
    private string _proxyStatus = string.Empty;

    public SettingsViewModel(
        ISettingsService settings,
        IYtDlpPathService paths,
        IFfmpegPathService ffmpegPaths,
        IYtDlpCli cli)
    {
        _settings = settings;
        _paths = paths;
        _ffmpegPaths = ffmpegPaths;
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

        _ffmpegPath = settings.Settings.FfmpegPath ?? string.Empty;
        _cookieFolder = settings.Settings.CookieFolder ?? string.Empty;

        _proxyEnabled = settings.Settings.ProxyEnabled;
        _proxyHost = settings.Settings.ProxyHost ?? string.Empty;
        _proxyPort = settings.Settings.ProxyPort ?? string.Empty;
        _proxyListMode = settings.Settings.ProxyListMode;
        _selectedProxyProtocol = ProxyProtocolCatalog.FirstOrDefault(
            option => option.Value == settings.Settings.ProxyProtocol) ?? ProxyProtocolCatalog[0];
        ProxySites = new ObservableCollection<string>(
            (settings.Settings.ProxySites ?? new List<string>())
                .Where(site => !string.IsNullOrWhiteSpace(site))
                .Select(site => site.Trim())
                .Distinct(StringComparer.OrdinalIgnoreCase));

        BrowseCommand = new RelayCommand(Browse);
        RescanCommand = new RelayCommand(Rescan);
        BrowseFfmpegCommand = new RelayCommand(BrowseFfmpeg);
        ClearFfmpegCommand = new RelayCommand(
            () => FfmpegPath = string.Empty,
            () => !string.IsNullOrWhiteSpace(_ffmpegPath));
        BrowseCookieFolderCommand = new RelayCommand(BrowseCookieFolder);
        ClearCookieFolderCommand = new RelayCommand(
            () => CookieFolder = string.Empty,
            () => !string.IsNullOrWhiteSpace(_cookieFolder));
        AddProxySiteCommand = new RelayCommand(AddProxySite, CanAddProxySite);
        RemoveProxySiteCommand = new RelayCommand<string>(RemoveProxySite);

        RefreshFfmpegStatus();
        RefreshCookieFolderStatus();
        RefreshProxyStatus();
        _ = RefreshDetectionAsync();
    }

    public RelayCommand BrowseCommand { get; }
    public RelayCommand RescanCommand { get; }
    public RelayCommand BrowseFfmpegCommand { get; }
    public RelayCommand ClearFfmpegCommand { get; }
    public RelayCommand BrowseCookieFolderCommand { get; }
    public RelayCommand ClearCookieFolderCommand { get; }

    public RelayCommand AddProxySiteCommand { get; }
    public RelayCommand<string> RemoveProxySiteCommand { get; }

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

    /// <summary>Manually specified ffmpeg executable. Empty means auto-detect from PATH.</summary>
    public string FfmpegPath
    {
        get => _ffmpegPath;
        set
        {
            var newValue = value ?? string.Empty;
            if (!SetProperty(ref _ffmpegPath, newValue))
                return;

            _settings.Settings.FfmpegPath = newValue.Trim();
            _settings.Save();
            RefreshFfmpegStatus();
            ClearFfmpegCommand.NotifyCanExecuteChanged();
        }
    }

    /// <summary>Human-readable summary of how ffmpeg was located (PATH, manual, or missing).</summary>
    public string FfmpegStatus
    {
        get => _ffmpegStatus;
        private set => SetProperty(ref _ffmpegStatus, value);
    }

    /// <summary>True when ffmpeg is not on PATH, so the manual path controls should be shown.</summary>
    public bool ShowFfmpegManualControls
    {
        get => _showFfmpegManualControls;
        private set => SetProperty(ref _showFfmpegManualControls, value);
    }

    /// <summary>Folder scanned for cookie files. Empty means cookies are not used.</summary>
    public string CookieFolder
    {
        get => _cookieFolder;
        set
        {
            var newValue = value ?? string.Empty;
            if (!SetProperty(ref _cookieFolder, newValue))
                return;

            _settings.Settings.CookieFolder = newValue.Trim();
            _settings.Save();
            RefreshCookieFolderStatus();
            ClearCookieFolderCommand.NotifyCanExecuteChanged();
        }
    }

    /// <summary>Human-readable summary of how many cookie files the folder contains.</summary>
    public string CookieFolderStatus
    {
        get => _cookieFolderStatus;
        private set => SetProperty(ref _cookieFolderStatus, value);
    }

    /// <summary>Master switch: when off the proxy is never added to any command.</summary>
    public bool ProxyEnabled
    {
        get => _proxyEnabled;
        set
        {
            if (!SetProperty(ref _proxyEnabled, value))
                return;

            _settings.Settings.ProxyEnabled = value;
            _settings.Save();
            RefreshProxyStatus();
        }
    }

    /// <summary>Protocol choices offered in the proxy card.</summary>
    public IReadOnlyList<ProxyProtocolOption> ProxyProtocolOptions => ProxyProtocolCatalog;

    /// <summary>Selected proxy protocol prefix.</summary>
    public ProxyProtocolOption SelectedProxyProtocol
    {
        get => _selectedProxyProtocol;
        set
        {
            if (value is null || ReferenceEquals(_selectedProxyProtocol, value))
                return;

            SetProperty(ref _selectedProxyProtocol, value);
            _settings.Settings.ProxyProtocol = value.Value;
            _settings.Save();
            RefreshProxyStatus();
        }
    }

    /// <summary>Proxy host or IP address.</summary>
    public string ProxyHost
    {
        get => _proxyHost;
        set
        {
            if (!SetProperty(ref _proxyHost, value ?? string.Empty))
                return;

            _settings.Settings.ProxyHost = _proxyHost.Trim();
            _settings.Save();
            RefreshProxyStatus();
        }
    }

    /// <summary>Proxy port, validated as 1-65535 when building the proxy URL.</summary>
    public string ProxyPort
    {
        get => _proxyPort;
        set
        {
            if (!SetProperty(ref _proxyPort, value ?? string.Empty))
                return;

            _settings.Settings.ProxyPort = _proxyPort.Trim();
            _settings.Save();
            RefreshProxyStatus();
        }
    }

    /// <summary>True when the site list is a whitelist (only listed sites use the proxy).</summary>
    public bool IsWhitelistMode
    {
        get => _proxyListMode == ProxyListMode.Whitelist;
        set { if (value) SetProxyListMode(ProxyListMode.Whitelist); }
    }

    /// <summary>True when the site list is a blacklist (listed sites never use the proxy).</summary>
    public bool IsBlacklistMode
    {
        get => _proxyListMode == ProxyListMode.Blacklist;
        set { if (value) SetProxyListMode(ProxyListMode.Blacklist); }
    }

    /// <summary>Websites the proxy rule applies to.</summary>
    public ObservableCollection<string> ProxySites { get; }

    /// <summary>Text currently typed into the add-website box.</summary>
    public string NewProxySite
    {
        get => _newProxySite;
        set
        {
            if (!SetProperty(ref _newProxySite, value ?? string.Empty))
                return;

            AddProxySiteCommand.NotifyCanExecuteChanged();
        }
    }

    /// <summary>Human-readable summary of the effective proxy rule.</summary>
    public string ProxyStatus
    {
        get => _proxyStatus;
        private set => SetProperty(ref _proxyStatus, value);
    }

    public string StatusText
    {
        get => _statusText;
        private set => SetProperty(ref _statusText, value);
    }

    private void SetProxyListMode(ProxyListMode mode)
    {
        if (_proxyListMode == mode)
            return;

        _proxyListMode = mode;
        _settings.Settings.ProxyListMode = mode;
        _settings.Save();
        OnPropertyChanged(nameof(IsWhitelistMode));
        OnPropertyChanged(nameof(IsBlacklistMode));
        RefreshProxyStatus();
    }

    private bool CanAddProxySite() => !string.IsNullOrWhiteSpace(_newProxySite);

    private void AddProxySite()
    {
        var site = _newProxySite.Trim();
        if (site.Length == 0)
            return;

        if (!ProxySites.Contains(site, StringComparer.OrdinalIgnoreCase))
            ProxySites.Add(site);

        NewProxySite = string.Empty;
        SaveProxySites();
    }

    private void RemoveProxySite(string site)
    {
        if (string.IsNullOrEmpty(site))
            return;

        if (ProxySites.Remove(site))
            SaveProxySites();
    }

    private void SaveProxySites()
    {
        _settings.Settings.ProxySites = ProxySites.ToList();
        _settings.Save();
        RefreshProxyStatus();
    }

    private void RefreshProxyStatus()
    {
        if (!_proxyEnabled)
        {
            ProxyStatus = "代理总开关已关闭：下载与解析都不会添加代理参数。";
            return;
        }

        var proxyUrl = ProxyRules.BuildProxyUrl(_settings.Settings);
        if (proxyUrl is null)
        {
            ProxyStatus = "请在下方填写有效的 IP / 主机与端口（1-65535）。";
            return;
        }

        var count = ProxySites.Count;
        var scope = _proxyListMode == ProxyListMode.Whitelist
            ? count == 0
                ? "白名单为空：不会对任何网站使用代理。"
                : $"白名单模式：仅对名单中的 {count} 个网站使用代理。"
            : count == 0
                ? "黑名单为空：所有网站都会使用代理。"
                : $"黑名单模式：对名单外的所有网站使用代理（已排除 {count} 个网站）。";

        ProxyStatus = $"代理已启用：{proxyUrl}，{scope}";
    }

    private void RefreshFfmpegStatus()
    {
        var located = _ffmpegPaths.LocateInPath();
        ShowFfmpegManualControls = located is null;

        if (located is not null)
        {
            FfmpegStatus = $"已在系统 PATH 中检测到 ffmpeg：{located}（无需手动配置）";
            return;
        }

        FfmpegStatus = !string.IsNullOrWhiteSpace(_ffmpegPath)
            ? File.Exists(_ffmpegPath)
                ? $"已配置 ffmpeg：{_ffmpegPath}"
                : "指定的 ffmpeg 路径不存在，请重新选择。"
            : "未在系统 PATH 中检测到 ffmpeg。合成音视频需要它，请点击“浏览…”手动指定 ffmpeg.exe 的路径。";
    }

    private void RefreshCookieFolderStatus()
    {
        if (string.IsNullOrWhiteSpace(_cookieFolder))
        {
            CookieFolderStatus = "未设置：下载页将不使用 Cookie。";
            return;
        }

        if (!Directory.Exists(_cookieFolder))
        {
            CookieFolderStatus = "文件夹不存在，请重新选择。";
            return;
        }

        int count;
        try
        {
            count = Directory.EnumerateFiles(_cookieFolder).Count();
        }
        catch (Exception ex)
        {
            CookieFolderStatus = $"无法读取文件夹：{ex.Message}";
            return;
        }

        CookieFolderStatus = count == 0
            ? "该文件夹内未找到 Cookie 文件。"
            : $"已识别 {count} 个 Cookie 文件，可在下载页的下拉列表中选择。";
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
        RefreshFfmpegStatus();
    }

    private void BrowseFfmpeg()
    {
        var dialog = new Microsoft.Win32.OpenFileDialog
        {
            Title = "选择 ffmpeg 可执行文件",
            Filter = "ffmpeg 可执行文件 (ffmpeg.exe)|ffmpeg.exe|所有文件 (*.*)|*.*",
            CheckFileExists = true,
        };

        var owner = System.Windows.Application.Current?.MainWindow;
        bool? result = owner is not null ? dialog.ShowDialog(owner) : dialog.ShowDialog();
        if (result == true)
            FfmpegPath = dialog.FileName;
    }

    private void BrowseCookieFolder()
    {
        var dialog = new System.Windows.Forms.FolderBrowserDialog
        {
            Description = "选择存放 Cookie 文件的文件夹",
            SelectedPath = Directory.Exists(CookieFolder) ? CookieFolder : string.Empty,
            UseDescriptionForTitle = true,
        };

        var owner = System.Windows.Application.Current?.MainWindow;
        System.Windows.Forms.DialogResult result;
        if (owner is not null)
            result = dialog.ShowDialog(new Win32Window(new System.Windows.Interop.WindowInteropHelper(owner).Handle));
        else
            result = dialog.ShowDialog();

        if (result == System.Windows.Forms.DialogResult.OK)
            CookieFolder = dialog.SelectedPath;
    }

    private sealed class Win32Window : System.Windows.Forms.IWin32Window
    {
        public Win32Window(IntPtr handle) => Handle = handle;
        public IntPtr Handle { get; }
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
