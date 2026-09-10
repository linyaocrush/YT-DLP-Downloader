using System.Collections.ObjectModel;
using System.IO;
using System.Net.Http;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using YtDlpDownloader.Core.Mvvm;
using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.App.ViewModels;

public sealed class DownloadViewModel : ObservableObject
{
    private const string AutoFormatExpression = "bv*+ba/b";
    private const int MaxLogLines = 1000;

    private static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(15) };

    private readonly ISettingsService _settings;
    private readonly IYtDlpCli _cli;

    private CancellationTokenSource? _cts;

    private bool _isBusy;
    private bool _isManualMode;
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

    private VideoSource? _selectedVideo;
    private AudioSource? _selectedAudio;

    public DownloadViewModel(ISettingsService settings, IYtDlpCli cli)
    {
        _settings = settings;
        _cli = cli;

        _downloadDirectory = string.IsNullOrWhiteSpace(settings.Settings.DownloadDirectory)
            ? GetDefaultDownloadDirectory()
            : settings.Settings.DownloadDirectory;

        VideoSources = new ObservableCollection<VideoSource>();
        AudioSources = new ObservableCollection<AudioSource>();
        Logs = new ObservableCollection<string>();

        ParseCommand = new AsyncRelayCommand(ParseAsync, CanParse);
        DownloadCommand = new AsyncRelayCommand(DownloadAsync, CanDownload);
        CancelCommand = new RelayCommand(Cancel, CanCancel);
        BrowseFolderCommand = new RelayCommand(BrowseFolder);

        RefreshYtDlpStatus();
        StatusText = "就绪：输入视频链接后点击“解析”，即可查看可用的画质与音质源。";
        UpdateSelectionHint();
    }

    public ObservableCollection<VideoSource> VideoSources { get; }
    public ObservableCollection<AudioSource> AudioSources { get; }
    public ObservableCollection<string> Logs { get; }

    public AsyncRelayCommand ParseCommand { get; }
    public AsyncRelayCommand DownloadCommand { get; }
    public RelayCommand CancelCommand { get; }
    public RelayCommand BrowseFolderCommand { get; }

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
        => SelectedVideo is not null
           && (SelectedVideo.HasAudio || SelectedAudio is not null);

    private string ManualExpression
        => SelectedVideo!.HasAudio
            ? SelectedVideo.FormatId
            : $"{SelectedVideo.FormatId}+{SelectedAudio!.FormatId}";

    private void UpdateSelectionHint()
    {
        SelectionHint = IsManualMode
            ? ManualSelectionValid
                ? SelectedVideo!.HasAudio
                    ? $"已选视频源 {SelectedVideo.FormatId}（音视频合一，单文件下载）"
                    : $"已选：{SelectedVideo.FormatId} + {SelectedAudio!.FormatId}"
                : "手动模式：请从上方列表选择一个视频源；纯视频源需再选择一个音频源用于合成。"
            : "自动模式：将自动挑选最高分辨率的视频源与最高音质的音频源，由 yt-dlp 合成为完整视频。";
    }

    private async Task ParseAsync()
    {
        SetBusy(true);
        _cts = new CancellationTokenSource();
        StatusText = "正在解析视频信息…";

        try
        {
            var url = Url.Trim();
            var info = await _cli.GetMediaInfoAsync(url, _cts.Token);

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
        var expression = IsAutoMode ? AutoFormatExpression : ManualExpression;
        var directory = string.IsNullOrWhiteSpace(DownloadDirectory)
            ? GetDefaultDownloadDirectory()
            : DownloadDirectory.Trim();

        SetBusy(true);
        _cts = new CancellationTokenSource();
        Progress = 0;
        IsProgressVisible = true;
        Logs.Clear();

        StatusText = IsAutoMode
            ? "开始下载（自动选择最佳质量并合成）…"
            : $"开始下载（格式 {expression}）…";

        try
        {
            var progress = new Progress<DownloadUpdate>(ApplyUpdate);
            var outcome = await _cli.DownloadAsync(Url.Trim(), expression, directory, progress, _cts.Token);

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

    private sealed class Win32Window : System.Windows.Forms.IWin32Window
    {
        public Win32Window(IntPtr handle) => Handle = handle;
        public IntPtr Handle { get; }
    }
}
