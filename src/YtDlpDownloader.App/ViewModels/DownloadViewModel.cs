using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Linq;
using System.Net.Http;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using YtDlpDownloader.Core.Mvvm;
using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.App.ViewModels;

/// <summary>A selectable auto-mode resolution preference and its display label.</summary>
public sealed record ResolutionOption(ResolutionPreference Value, string Label);

/// <summary>A selectable cookie file (or the "none" entry) shown in the download view.</summary>
public sealed record CookieOption(string? Path, string Label)
{
    /// <summary>The default entry meaning "do not send cookies".</summary>
    public static CookieOption None { get; } = new(null, "无（不使用 Cookie）");
}

public sealed class DownloadViewModel : ObservableObject
{
    private const string AutoFormatExpression = "bv*+ba/b";
    private const int MaxLogLines = 1000;

    /// <summary>yt-dlp output-template fields offered as chip buttons.</summary>
    private static readonly (string Token, string Label)[] TemplateFieldCatalog =
    {
        ("%(title)s", "标题"),
        ("%(id)s", "视频 ID"),
        ("%(uploader)s", "上传者"),
        ("%(upload_date)s", "上传日期"),
        ("%(resolution)s", "分辨率"),
        ("%(fps)s", "帧率"),
        ("%(duration)s", "时长"),
        ("%(extractor)s", "站点"),
        ("%(format_id)s", "格式 ID"),
        ("%(vcodec)s", "视频编码"),
        ("%(acodec)s", "音频编码"),
    };

    private static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(15) };

    /// <summary>Resolution choices offered in auto mode (labels use ≤ / > symbols).</summary>
    public static readonly IReadOnlyList<ResolutionOption> ResolutionCatalog = new[]
    {
        new ResolutionOption(ResolutionPreference.Best, "最佳"),
        new ResolutionOption(ResolutionPreference.Above4K, "> 4K"),
        new ResolutionOption(ResolutionPreference.UpTo4K, "≤ 4K"),
        new ResolutionOption(ResolutionPreference.UpTo2K, "≤ 2K"),
        new ResolutionOption(ResolutionPreference.UpTo1080P, "≤ 1080P"),
        new ResolutionOption(ResolutionPreference.UpTo720P, "≤ 720P"),
        new ResolutionOption(ResolutionPreference.UpTo360P, "≤ 360P"),
    };

    private readonly ISettingsService _settings;
    private readonly IYtDlpCli _cli;

    private CancellationTokenSource? _cts;

    private bool _isBusy;
    private bool _isManualMode;
    private DownloadKind _downloadKind = DownloadKind.Default;
    private ResolutionOption _selectedResolution = ResolutionCatalog[0];
    private bool _isTemplateNameMode = true;
    private bool _isProgressVisible;
    private bool _hasMediaInfo;
    private double _progress;

    private ImageSource? _thumbnail;

    private string _url = string.Empty;
    private string _downloadDirectory = string.Empty;
    private string _statusText = string.Empty;
    private string _ytDlpStatus = string.Empty;
    private string _videoTitle = string.Empty;
    private string _metaText = string.Empty;
    private string _selectionHint = string.Empty;
    private string _fixedFileName = string.Empty;
    private string _outputTemplate = "%(title)s.%(ext)s";
    private int _downloadThreads = 4;

    private VideoSource? _selectedVideo;
    private AudioSource? _selectedAudio;

    private CookieOption _selectedCookie = CookieOption.None;
    private string _cookieFolder = string.Empty;
    private string _cookieHint = string.Empty;
    private bool _cookieNeedsSetup;
    private bool _hasCookieFolder;

    private bool _proxyWillApply;
    private string _proxyStatusText = string.Empty;
    private string _proxyStatusDetail = string.Empty;

    private static readonly System.Windows.Media.Brush ProxyOnBrush = CreateFrozenBrush(0x0F, 0x7B, 0x0F);
    private static readonly System.Windows.Media.Brush ProxyOffBrush = CreateFrozenBrush(0x9A, 0x9A, 0x9A);

    public DownloadViewModel(ISettingsService settings, IYtDlpCli cli)
    {
        _settings = settings;
        _cli = cli;

        _downloadDirectory = string.IsNullOrWhiteSpace(settings.Settings.DownloadDirectory)
            ? GetDefaultDownloadDirectory()
            : settings.Settings.DownloadDirectory;
        _downloadThreads = settings.Settings.DownloadThreads >= 1
            ? settings.Settings.DownloadThreads
            : 4;
        _isTemplateNameMode = settings.Settings.OutputNameMode != OutputNameMode.Fixed;
        _fixedFileName = settings.Settings.OutputFileName ?? string.Empty;
        _downloadKind = settings.Settings.DownloadKind;
        _selectedResolution = ResolutionCatalog.FirstOrDefault(
            option => option.Value == settings.Settings.Resolution) ?? ResolutionCatalog[0];

        VideoSources = new ObservableCollection<VideoSource>();
        AudioSources = new ObservableCollection<AudioSource>();
        Logs = new ObservableCollection<string>();
        OutputTemplateFields = new ObservableCollection<OutputTemplateField>();
        CookieOptions = new ObservableCollection<CookieOption>();

        var savedTemplate = settings.Settings.OutputTemplate ?? string.Empty;
        foreach (var (token, label) in TemplateFieldCatalog)
        {
            var field = new OutputTemplateField(token, label, IsTokenSelected(savedTemplate, token));
            field.PropertyChanged += OnTemplateFieldChanged;
            OutputTemplateFields.Add(field);
        }

        RebuildOutputTemplate();

        ParseCommand = new AsyncRelayCommand(ParseAsync, CanParse);
        DownloadCommand = new AsyncRelayCommand(DownloadAsync, CanDownload);
        CancelCommand = new RelayCommand(Cancel, CanCancel);
        BrowseFolderCommand = new RelayCommand(BrowseFolder);
        RefreshCookiesCommand = new RelayCommand(RefreshCookieOptions);

        _settings.Changed += OnSettingsChanged;
        RefreshCookieOptions();
        RefreshProxyStatus();

        RefreshYtDlpStatus();
        StatusText = "就绪：输入视频链接后点击“解析”，即可查看可用的画质与音质源。";
        UpdateSelectionHint();
    }

    public ObservableCollection<VideoSource> VideoSources { get; }
    public ObservableCollection<AudioSource> AudioSources { get; }
    public ObservableCollection<string> Logs { get; }
    public ObservableCollection<OutputTemplateField> OutputTemplateFields { get; }

    /// <summary>Cookie files discovered in the configured folder, plus a leading "none" entry.</summary>
    public ObservableCollection<CookieOption> CookieOptions { get; }

    public AsyncRelayCommand ParseCommand { get; }
    public AsyncRelayCommand DownloadCommand { get; }
    public RelayCommand CancelCommand { get; }
    public RelayCommand BrowseFolderCommand { get; }
    public RelayCommand RefreshCookiesCommand { get; }

    public string Url
    {
        get => _url;
        set
        {
            var newValue = value ?? string.Empty;
            if (string.Equals(_url, newValue, StringComparison.Ordinal))
                return;

            _url = newValue;
            OnPropertyChanged(nameof(Url));
            ClearParseResult();
            RefreshProxyStatus();
            NotifyStateChanged();
        }
    }

    public string DownloadDirectory
    {
        get => _downloadDirectory;
        set
        {
            if (!SetProperty(ref _downloadDirectory, value ?? string.Empty))
                return;

            _settings.Settings.DownloadDirectory = _downloadDirectory.Trim();
            _settings.Save();
        }
    }

    /// <summary>Currently selected cookie file (or <see cref="CookieOption.None"/>).</summary>
    public CookieOption SelectedCookie
    {
        get => _selectedCookie;
        set
        {
            if (value is null
                || string.Equals(_selectedCookie.Path, value.Path, StringComparison.OrdinalIgnoreCase))
                return;

            _selectedCookie = value;
            OnPropertyChanged();

            _settings.Settings.CookieFile = value.Path ?? string.Empty;
            _settings.Save();
        }
    }

    /// <summary>True when a valid cookie folder is configured.</summary>
    public bool HasCookieFolder
    {
        get => _hasCookieFolder;
        private set => SetProperty(ref _hasCookieFolder, value);
    }

    /// <summary>True when the user must configure a cookie folder in Settings.</summary>
    public bool CookieNeedsSetup
    {
        get => _cookieNeedsSetup;
        private set => SetProperty(ref _cookieNeedsSetup, value);
    }

    /// <summary>Contextual hint shown under the cookie selector.</summary>
    public string CookieHint
    {
        get => _cookieHint;
        private set => SetProperty(ref _cookieHint, value);
    }

    /// <summary>The path forwarded to yt-dlp via --cookies (empty when "none").</summary>
    private string CookieArgument => _selectedCookie.Path ?? string.Empty;

    /// <summary>The proxy URL forwarded to yt-dlp via --proxy (empty when unused).</summary>
    private string ProxyArgument => _proxyWillApply
        ? ProxyRules.BuildProxyUrl(_settings.Settings) ?? string.Empty
        : string.Empty;

    /// <summary>True when the current URL will be downloaded through the proxy.</summary>
    public bool ProxyWillApply
    {
        get => _proxyWillApply;
        private set => SetProperty(ref _proxyWillApply, value);
    }

    /// <summary>Short label for the proxy status indicator on the download page.</summary>
    public string ProxyStatusText
    {
        get => _proxyStatusText;
        private set => SetProperty(ref _proxyStatusText, value);
    }

    /// <summary>Longer explanation shown as the indicator's tooltip.</summary>
    public string ProxyStatusDetail
    {
        get => _proxyStatusDetail;
        private set => SetProperty(ref _proxyStatusDetail, value);
    }

    /// <summary>Indicator colour: green when the proxy applies, grey otherwise.</summary>
    public System.Windows.Media.Brush ProxyStatusBrush => _proxyWillApply ? ProxyOnBrush : ProxyOffBrush;

    /// <summary>Recomputes whether the current URL will use the proxy (settings or URL changed).</summary>
    public void RefreshProxyStatus()
    {
        var settings = _settings.Settings;
        var proxyUrl = ProxyRules.BuildProxyUrl(settings);
        ProxyWillApply = ProxyRules.ShouldUseProxy(settings, Url);

        if (proxyUrl is null)
        {
            ProxyStatusText = settings.ProxyEnabled ? "代理：未配置完整" : "代理：未启用";
            ProxyStatusDetail = settings.ProxyEnabled
                ? "已在设置中开启代理，但 IP 或端口无效，本次不会使用代理。"
                : "代理总开关已关闭，本次下载与解析都不会使用代理。";
        }
        else if (ProxyWillApply)
        {
            ProxyStatusText = "代理：将使用";
            ProxyStatusDetail = $"本次下载将通过 {proxyUrl} 访问网络。";
        }
        else
        {
            ProxyStatusText = "代理：不使用";
            ProxyStatusDetail = $"当前链接不在代理规则范围内，本次将直连（代理配置 {proxyUrl}）。";
        }

        OnPropertyChanged(nameof(ProxyStatusBrush));
    }

    /// <summary>Number of fragments downloaded concurrently (1-64).</summary>
    public int DownloadThreads
    {
        get => _downloadThreads;
        set
        {
            var clamped = Math.Clamp(value, 1, 64);
            if (!SetProperty(ref _downloadThreads, clamped))
                return;

            _settings.Settings.DownloadThreads = clamped;
            _settings.Save();
        }
    }

    /// <summary>True when the output name is composed from template field chips.</summary>
    public bool IsTemplateNameMode
    {
        get => _isTemplateNameMode;
        set { if (value) SetTemplateNameMode(true); }
    }

    /// <summary>True when the user types a fixed output file name.</summary>
    public bool IsFixedNameMode
    {
        get => !_isTemplateNameMode;
        set { if (value) SetTemplateNameMode(false); }
    }

    /// <summary>Read-only preview of the assembled yt-dlp output template.</summary>
    public string OutputTemplate
    {
        get => _outputTemplate;
        private set => SetProperty(ref _outputTemplate, value);
    }

    /// <summary>Base file name used in fixed-name mode (extension is appended automatically).</summary>
    public string FixedFileName
    {
        get => _fixedFileName;
        set
        {
            if (!SetProperty(ref _fixedFileName, value ?? string.Empty))
                return;

            _settings.Settings.OutputFileName = _fixedFileName.Trim();
            _settings.Save();
        }
    }

    public string OutputNameHint => IsTemplateNameMode
        ? "模板模式：点击下方字段按钮拼装文件名，再次点击可移除；扩展名（.%(ext)s）会自动添加。"
        : "文件名模式：直接输入文件名即可，无需填写扩展名，程序会自动添加。";

    public bool IsBusy
    {
        get => _isBusy;
        private set
        {
            if (SetProperty(ref _isBusy, value))
                NotifyStateChanged();
        }
    }

    public bool IsAutoMode
    {
        get => !_isManualMode;
        set { if (value) SetManualMode(false); }
    }

    public bool IsManualMode
    {
        get => _isManualMode;
        set => SetManualMode(value);
    }

    /// <summary>Download both video and audio (default).</summary>
    public bool IsDefaultKind
    {
        get => _downloadKind == DownloadKind.Default;
        set { if (value) SetDownloadKind(DownloadKind.Default); }
    }

    /// <summary>Download the video stream only.</summary>
    public bool IsVideoOnlyKind
    {
        get => _downloadKind == DownloadKind.VideoOnly;
        set { if (value) SetDownloadKind(DownloadKind.VideoOnly); }
    }

    /// <summary>Download the audio stream only.</summary>
    public bool IsAudioOnlyKind
    {
        get => _downloadKind == DownloadKind.AudioOnly;
        set { if (value) SetDownloadKind(DownloadKind.AudioOnly); }
    }

    /// <summary>True when the video source list should be shown in manual mode.</summary>
    public bool ShowVideoSources => _downloadKind != DownloadKind.AudioOnly;

    /// <summary>True when the audio source list should be shown in manual mode.</summary>
    public bool ShowAudioSources => _downloadKind != DownloadKind.VideoOnly;

    /// <summary>True when both source lists are visible (default kind).</summary>
    public bool ShowBothSources => ShowVideoSources && ShowAudioSources;

    /// <summary>Available resolution choices for auto mode.</summary>
    public IReadOnlyList<ResolutionOption> ResolutionOptions => ResolutionCatalog;

    /// <summary>Resolution preference used to constrain the auto-selected video format.</summary>
    public ResolutionOption SelectedResolution
    {
        get => _selectedResolution;
        set
        {
            if (value is null || ReferenceEquals(_selectedResolution, value))
                return;

            _selectedResolution = value;
            OnPropertyChanged();
            _settings.Settings.Resolution = value.Value;
            _settings.Save();
            UpdateSelectionHint();
        }
    }

    public string StatusText
    {
        get => _statusText;
        private set => SetProperty(ref _statusText, value);
    }

    public string YtDlpStatus
    {
        get => _ytDlpStatus;
        private set => SetProperty(ref _ytDlpStatus, value);
    }

    public string VideoTitle
    {
        get => _videoTitle;
        private set => SetProperty(ref _videoTitle, value);
    }

    public string MetaText
    {
        get => _metaText;
        private set => SetProperty(ref _metaText, value);
    }

    /// <summary>Cover image kept in memory only (never written to disk).</summary>
    public ImageSource? Thumbnail
    {
        get => _thumbnail;
        private set
        {
            if (SetProperty(ref _thumbnail, value))
                OnPropertyChanged(nameof(HasThumbnail));
        }
    }

    public bool HasThumbnail => _thumbnail is not null;

    public bool HasMediaInfo
    {
        get => _hasMediaInfo;
        private set => SetProperty(ref _hasMediaInfo, value);
    }

    public string SelectionHint
    {
        get => _selectionHint;
        private set => SetProperty(ref _selectionHint, value);
    }

    public double Progress
    {
        get => _progress;
        private set
        {
            if (SetProperty(ref _progress, value))
                OnPropertyChanged(nameof(ProgressText));
        }
    }

    public string ProgressText => $"{Progress:0}%";

    public bool IsProgressVisible
    {
        get => _isProgressVisible;
        private set => SetProperty(ref _isProgressVisible, value);
    }

    public VideoSource? SelectedVideo
    {
        get => _selectedVideo;
        set
        {
            if (SetProperty(ref _selectedVideo, value))
            {
                UpdateSelectionHint();
                NotifyStateChanged();
            }
        }
    }

    public AudioSource? SelectedAudio
    {
        get => _selectedAudio;
        set
        {
            if (SetProperty(ref _selectedAudio, value))
            {
                UpdateSelectionHint();
                NotifyStateChanged();
            }
        }
    }

    private void SetManualMode(bool manual)
    {
        if (_isManualMode == manual)
            return;

        _isManualMode = manual;
        OnPropertyChanged(nameof(IsManualMode));
        OnPropertyChanged(nameof(IsAutoMode));
        UpdateSelectionHint();
        NotifyStateChanged();
    }

    private void SetDownloadKind(DownloadKind kind)
    {
        if (_downloadKind == kind)
            return;

        _downloadKind = kind;
        _settings.Settings.DownloadKind = kind;
        _settings.Save();

        OnPropertyChanged(nameof(IsDefaultKind));
        OnPropertyChanged(nameof(IsVideoOnlyKind));
        OnPropertyChanged(nameof(IsAudioOnlyKind));
        OnPropertyChanged(nameof(ShowVideoSources));
        OnPropertyChanged(nameof(ShowAudioSources));
        OnPropertyChanged(nameof(ShowBothSources));

        UpdateSelectionHint();
        NotifyStateChanged();
    }

    private void SetTemplateNameMode(bool template)
    {
        if (_isTemplateNameMode == template)
            return;

        _isTemplateNameMode = template;
        _settings.Settings.OutputNameMode = template ? OutputNameMode.Template : OutputNameMode.Fixed;
        _settings.Save();
        OnPropertyChanged(nameof(IsTemplateNameMode));
        OnPropertyChanged(nameof(IsFixedNameMode));
        OnPropertyChanged(nameof(OutputNameHint));
    }

    private static bool IsTokenSelected(string template, string token)
        => !string.IsNullOrEmpty(template)
           && template.Contains(token, StringComparison.OrdinalIgnoreCase);

    private void OnTemplateFieldChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(OutputTemplateField.IsSelected))
            RebuildOutputTemplate();
    }

    private void RebuildOutputTemplate()
    {
        var selected = OutputTemplateFields.Where(field => field.IsSelected).ToList();
        if (selected.Count == 0)
        {
            var title = OutputTemplateFields.FirstOrDefault(
                field => string.Equals(field.Token, "%(title)s", StringComparison.OrdinalIgnoreCase));
            if (title is not null)
            {
                title.IsSelected = true;
                return;
            }
        }

        var fields = string.Join(" ", selected.Select(field => field.Token));
        OutputTemplate = $"{fields}.%(ext)s";
        _settings.Settings.OutputTemplate = fields;
        _settings.Save();
    }

    private string? BuildOutputTemplate()
    {
        if (IsFixedNameMode)
        {
            var name = _fixedFileName.Trim();
            if (name.Length == 0)
                return null;

            return name.Contains("%(ext)s", StringComparison.OrdinalIgnoreCase)
                ? name
                : $"{name}.%(ext)s";
        }

        return string.IsNullOrWhiteSpace(_outputTemplate) ? null : _outputTemplate;
    }

    public void RefreshYtDlpStatus()
    {
        YtDlpStatus = _cli.ResolveExecutable() is null
            ? "未检测到 yt-dlp，请到“设置”页配置"
            : "yt-dlp 已就绪";
        NotifyStateChanged();
    }

    public void ClearParseResult()
    {
        VideoSources.Clear();
        AudioSources.Clear();
        VideoTitle = string.Empty;
        MetaText = string.Empty;
        Thumbnail = null;
        HasMediaInfo = false;
        SelectedVideo = null;
        SelectedAudio = null;
    }

    /// <summary>Releases the in-memory cover image (called on application shutdown).</summary>
    public void Cleanup()
    {
        Thumbnail = null;
        Logs.Clear();
    }

    private void NotifyStateChanged()
    {
        ParseCommand.NotifyCanExecuteChanged();
        DownloadCommand.NotifyCanExecuteChanged();
        CancelCommand.NotifyCanExecuteChanged();
        UpdateSelectionHint();
    }

    private bool CanParse()
        => !IsBusy
           && !string.IsNullOrWhiteSpace(Url)
           && _cli.ResolveExecutable() is not null;

    private bool CanDownload()
        => !IsBusy
           && !string.IsNullOrWhiteSpace(Url)
           && _cli.ResolveExecutable() is not null
           && (IsAutoMode || ManualSelectionValid);

    private bool CanCancel() => IsBusy;

    private bool ManualSelectionValid
        => _downloadKind switch
        {
            DownloadKind.VideoOnly => SelectedVideo is not null,
            DownloadKind.AudioOnly => SelectedAudio is not null,
            _ => SelectedVideo is not null
                 && (SelectedVideo.HasAudio || SelectedAudio is not null),
        };

    private string ManualExpression
        => SelectedVideo!.HasAudio
            ? SelectedVideo.FormatId
            : $"{SelectedVideo.FormatId}+{SelectedAudio!.FormatId}";

    private string BuildFormatExpression()
    {
        if (IsAutoMode)
        {
            var filter = _downloadKind == DownloadKind.AudioOnly
                ? null
                : ResolutionFilter(_selectedResolution.Value);

            return _downloadKind switch
            {
                DownloadKind.VideoOnly => filter is null ? "bv" : $"bv{filter}",
                DownloadKind.AudioOnly => "ba",
                _ => filter is null ? AutoFormatExpression : $"bv*{filter}+ba/b{filter}",
            };
        }

        return _downloadKind switch
        {
            DownloadKind.VideoOnly => SelectedVideo!.FormatId,
            DownloadKind.AudioOnly => SelectedAudio!.FormatId,
            _ => ManualExpression,
        };
    }

    private static string? ResolutionFilter(ResolutionPreference preference) => preference switch
    {
        ResolutionPreference.Above4K => "[height>2160]",
        ResolutionPreference.UpTo4K => "[height<=2160]",
        ResolutionPreference.UpTo2K => "[height<=1440]",
        ResolutionPreference.UpTo1080P => "[height<=1080]",
        ResolutionPreference.UpTo720P => "[height<=720]",
        ResolutionPreference.UpTo360P => "[height<=360]",
        _ => null,
    };

    private void UpdateSelectionHint()
    {
        if (IsManualMode)
        {
            SelectionHint = _downloadKind switch
            {
                DownloadKind.VideoOnly => SelectedVideo is null
                    ? "仅视频：请从上方列表选择一个视频源（仅下载画面，不含声音）。"
                    : $"已选视频源 {SelectedVideo.FormatId}（仅视频，无声音）",
                DownloadKind.AudioOnly => SelectedAudio is null
                    ? "仅音频：请从上方列表选择一个音频源。"
                    : $"已选音频源 {SelectedAudio.FormatId}（仅音频）",
                _ => ManualSelectionValid
                    ? SelectedVideo!.HasAudio
                        ? $"已选视频源 {SelectedVideo.FormatId}（音视频合一，单文件下载）"
                        : $"已选：{SelectedVideo.FormatId} + {SelectedAudio!.FormatId}"
                    : "手动模式：请从上方列表选择一个视频源；纯视频源需再选择一个音频源用于合成。",
            };
            return;
        }

        SelectionHint = _downloadKind switch
        {
            DownloadKind.VideoOnly => $"仅视频：自动挑选最高画质视频源{ResolutionHintSuffix}，不下载声音。",
            DownloadKind.AudioOnly => "仅音频：自动挑选最高音质的音频源。",
            _ => $"自动模式：自动挑选最佳视频与音频并合成{ResolutionHintSuffix}。",
        };
    }

    private string ResolutionHintSuffix => _selectedResolution.Value == ResolutionPreference.Best
        ? string.Empty
        : $"（分辨率限制 {_selectedResolution.Label}）";

    private async Task ParseAsync()
    {
        SetBusy(true);
        _cts = new CancellationTokenSource();
        StatusText = "正在解析视频信息…";

        try
        {
            var url = Url.Trim();
            var info = await _cli.GetMediaInfoAsync(url, CookieArgument, ProxyArgument, _cts.Token);

            ClearParseResult();
            VideoTitle = info.Title;
            MetaText = $"ID: {info.Id}    时长: {info.DurationText}"
                + (string.IsNullOrEmpty(info.Uploader) ? string.Empty : $"    上传者: {info.Uploader}");
            HasMediaInfo = true;

            foreach (var video in info.Videos)
                VideoSources.Add(video);
            foreach (var audio in info.Audios)
                AudioSources.Add(audio);

            Thumbnail = await LoadThumbnailAsync(info.ThumbnailUrl, _cts.Token);

            StatusText = $"解析完成：{info.Title}（{VideoSources.Count} 个视频源，{AudioSources.Count} 个音频源）";
        }
        catch (OperationCanceledException)
        {
            StatusText = "解析已取消";
        }
        catch (YtDlpException ex)
        {
            StatusText = ex.Message;
        }
        catch (Exception ex)
        {
            StatusText = $"解析失败：{ex.Message}";
        }
        finally
        {
            _cts?.Dispose();
            _cts = null;
            SetBusy(false);
        }
    }

    private async Task DownloadAsync()
    {
        var expression = BuildFormatExpression();
        var directory = string.IsNullOrWhiteSpace(DownloadDirectory)
            ? GetDefaultDownloadDirectory()
            : DownloadDirectory.Trim();

        SetBusy(true);
        _cts = new CancellationTokenSource();
        Progress = 0;
        IsProgressVisible = true;
        Logs.Clear();

        StatusText = IsAutoMode
            ? _downloadKind switch
            {
                DownloadKind.VideoOnly => "开始下载（仅视频，自动选择最高画质）…",
                DownloadKind.AudioOnly => "开始下载（仅音频，自动选择最高音质）…",
                _ => "开始下载（自动选择最佳质量并合成）…",
            }
            : $"开始下载（格式 {expression}）…";

        try
        {
            var progress = new Progress<DownloadUpdate>(ApplyUpdate);
            var outcome = await _cli.DownloadAsync(
                Url.Trim(), expression, directory, CookieArgument, ProxyArgument, DownloadThreads, BuildOutputTemplate(), progress, _cts.Token);

            if (outcome.Cancelled)
            {
                StatusText = "下载已取消";
            }
            else if (outcome.Success)
            {
                StatusText = outcome.OutputPath is null
                    ? "下载完成"
                    : $"下载完成：{outcome.OutputPath}";
            }
            else
            {
                StatusText = $"下载失败：{outcome.Error ?? "未知错误"}";
                Logs.Add(outcome.Error ?? "未知错误");
            }
        }
        catch (OperationCanceledException)
        {
            StatusText = "下载已取消";
        }
        catch (Exception ex)
        {
            StatusText = $"下载失败：{ex.Message}";
        }
        finally
        {
            _cts?.Dispose();
            _cts = null;
            IsProgressVisible = false;
            SetBusy(false);
        }
    }

    private void ApplyUpdate(DownloadUpdate update)
    {
        if (update.Percent is double percent)
            Progress = percent;

        if (!string.IsNullOrEmpty(update.Line))
            AppendLog(update.Line);
    }

    private void Cancel()
    {
        StatusText = "正在取消…";
        _cts?.Cancel();
    }

    private void BrowseFolder()
    {
        var dialog = new System.Windows.Forms.FolderBrowserDialog
        {
            Description = "选择下载保存目录",
            SelectedPath = Directory.Exists(DownloadDirectory) ? DownloadDirectory : GetDefaultDownloadDirectory(),
            UseDescriptionForTitle = true,
        };

        var owner = Application.Current?.MainWindow;
        System.Windows.Forms.DialogResult result;
        if (owner is not null)
            result = dialog.ShowDialog(new Win32Window(new System.Windows.Interop.WindowInteropHelper(owner).Handle));
        else
            result = dialog.ShowDialog();

        if (result == System.Windows.Forms.DialogResult.OK)
            DownloadDirectory = dialog.SelectedPath;
    }

    private void OnSettingsChanged(object? sender, EventArgs e)
    {
        var folder = _settings.Settings.CookieFolder ?? string.Empty;
        if (!string.Equals(folder, _cookieFolder, StringComparison.OrdinalIgnoreCase))
            RefreshCookieOptions();

        RefreshProxyStatus();
    }

    /// <summary>Rebuilds the cookie dropdown from the configured folder and restores the selection.</summary>
    private void RefreshCookieOptions()
    {
        _cookieFolder = _settings.Settings.CookieFolder ?? string.Empty;
        var savedPath = _settings.Settings.CookieFile ?? string.Empty;
        var currentPath = _selectedCookie.Path;

        var options = new List<CookieOption> { CookieOption.None };
        if (Directory.Exists(_cookieFolder))
        {
            try
            {
                foreach (var file in Directory.EnumerateFiles(_cookieFolder)
                             .OrderBy(Path.GetFileName, StringComparer.OrdinalIgnoreCase))
                    options.Add(new CookieOption(file, Path.GetFileName(file)));
            }
            catch
            {
                // Unreadable folder: fall back to the "none" entry only.
            }
        }

        CookieOptions.Clear();
        foreach (var option in options)
            CookieOptions.Add(option);

        _selectedCookie = options.FirstOrDefault(
                              option => option.Path is not null
                                        && string.Equals(option.Path, savedPath, StringComparison.OrdinalIgnoreCase))
                          ?? options.FirstOrDefault(
                              option => option.Path is not null
                                        && string.Equals(option.Path, currentPath, StringComparison.OrdinalIgnoreCase))
                          ?? CookieOption.None;

        OnPropertyChanged(nameof(SelectedCookie));

        HasCookieFolder = Directory.Exists(_cookieFolder);
        CookieNeedsSetup = !HasCookieFolder;
        CookieHint = !string.IsNullOrWhiteSpace(_cookieFolder) && !HasCookieFolder
            ? "Cookie 文件夹不存在，请到“设置”页重新选择。"
            : string.IsNullOrWhiteSpace(_cookieFolder)
                ? "未设置 Cookie 文件夹。请到“设置”页选择一个文件夹，即可在此选择其中的 Cookie 文件。"
                : "可选：用于需要登录或年龄限制的视频，从上方下拉列表选择 Cookie 文件。";
    }

    private void SetBusy(bool busy) => IsBusy = busy;

    private void AppendLog(string line)
    {
        Logs.Add(line);
        if (Logs.Count > MaxLogLines)
            Logs.RemoveAt(0);
    }

    private static async Task<ImageSource?> LoadThumbnailAsync(string? url, CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(url))
            return null;

        try
        {
            var bytes = await Http.GetByteArrayAsync(url, cancellationToken);
            using var stream = new MemoryStream(bytes);

            var image = new BitmapImage();
            image.BeginInit();
            image.CacheOption = BitmapCacheOption.OnLoad;
            image.StreamSource = stream;
            image.DecodePixelWidth = 360;
            image.EndInit();
            image.Freeze();
            return image;
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch
        {
            return null;
        }
    }

    private static string GetDefaultDownloadDirectory()
    {
        var profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        var downloads = Path.Combine(profile, "Downloads");
        return Directory.Exists(downloads) ? downloads : profile;
    }

    private static System.Windows.Media.Brush CreateFrozenBrush(byte r, byte g, byte b)
    {
        var brush = new SolidColorBrush(System.Windows.Media.Color.FromRgb(r, g, b));
        brush.Freeze();
        return brush;
    }

    private sealed class Win32Window : System.Windows.Forms.IWin32Window
    {
        public Win32Window(IntPtr handle) => Handle = handle;
        public IntPtr Handle { get; }
    }
}
