using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Windows.Media;
using YtDlpDownloader.Core.Models;
using YtDlpDownloader.Core.Mvvm;

namespace YtDlpDownloader.App.ViewModels;

/// <summary>Lifecycle of a single item inside the in-page download queue.</summary>
public enum QueueItemState
{
    Pending,
    Parsing,
    Parsed,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

/// <summary>One row of the download queue shown on the download page.</summary>
public sealed class QueueItemViewModel : ObservableObject
{
    private string _title;
    private string? _thumbnailUrl;
    private ImageSource? _thumbnail;
    private QueueItemState _state = QueueItemState.Pending;
    private double _progress;
    private string? _outputPath;
    private string? _error;
    private VideoSource? _selectedVideo;
    private AudioSource? _selectedAudio;

    public QueueItemViewModel(string url)
    {
        Url = url;
        _title = url;
    }

    public string Url { get; }

    public string Title
    {
        get => _title;
        private set => SetProperty(ref _title, value);
    }

    public string? ThumbnailUrl
    {
        get => _thumbnailUrl;
        private set => SetProperty(ref _thumbnailUrl, value);
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

    /// <summary>Video sources parsed for this item (used for manual selection).</summary>
    public ObservableCollection<VideoSource> VideoSources { get; } = new();

    /// <summary>Audio sources parsed for this item (used for manual selection).</summary>
    public ObservableCollection<AudioSource> AudioSources { get; } = new();

    /// <summary>Video source manually chosen for this item.</summary>
    public VideoSource? SelectedVideo
    {
        get => _selectedVideo;
        set
        {
            if (SetProperty(ref _selectedVideo, value))
                NotifySelectionChanged();
        }
    }

    /// <summary>Audio source manually chosen for this item.</summary>
    public AudioSource? SelectedAudio
    {
        get => _selectedAudio;
        set
        {
            if (SetProperty(ref _selectedAudio, value))
                NotifySelectionChanged();
        }
    }

    /// <summary>Human-readable summary of the chosen sources (resolution/fps/bitrate, not format ids).</summary>
    public string SelectionInfo
    {
        get
        {
            var parts = new List<string>();

            if (_selectedVideo is not null)
            {
                var bits = new[] { _selectedVideo.Resolution, _selectedVideo.FpsText, _selectedVideo.BitrateText, _selectedVideo.Codec }
                    .Where(value => !string.IsNullOrWhiteSpace(value));
                parts.Add("视频：" + string.Join(" · ", bits));
            }

            if (_selectedAudio is not null)
            {
                var bits = new[] { _selectedAudio.BitrateText, _selectedAudio.Codec }
                    .Where(value => !string.IsNullOrWhiteSpace(value));
                parts.Add("音频：" + string.Join(" · ", bits));
            }

            return string.Join("    ", parts);
        }
    }

    public bool HasSelection => _selectedVideo is not null || _selectedAudio is not null;

    public QueueItemState State
    {
        get => _state;
        private set
        {
            if (SetProperty(ref _state, value))
                NotifyStateDependents();
        }
    }

    public double Progress
    {
        get => _progress;
        private set
        {
            if (SetProperty(ref _progress, value))
            {
                OnPropertyChanged(nameof(ProgressText));
                OnPropertyChanged(nameof(StatusText));
            }
        }
    }

    public string ProgressText => $"{Progress:0}%";

    public string? OutputPath
    {
        get => _outputPath;
        private set => SetProperty(ref _outputPath, value);
    }

    public string? Error
    {
        get => _error;
        private set => SetProperty(ref _error, value);
    }

    public bool IsPending => State == QueueItemState.Pending;
    public bool IsParsing => State == QueueItemState.Parsing;
    public bool IsParsed => State == QueueItemState.Parsed;
    public bool IsDownloading => State == QueueItemState.Downloading;
    public bool IsCompleted => State == QueueItemState.Completed;
    public bool IsFailed => State == QueueItemState.Failed;
    public bool IsCancelled => State == QueueItemState.Cancelled;

    /// <summary>True while parsing or downloading (drives the per-item progress bar).</summary>
    public bool ShowProgress => State == QueueItemState.Parsing || State == QueueItemState.Downloading;

    /// <summary>True when the item failed or was cancelled and can be retried.</summary>
    public bool CanRetry => State == QueueItemState.Failed || State == QueueItemState.Cancelled;

    /// <summary>Short state description shown under the title.</summary>
    public string StatusText => State switch
    {
        QueueItemState.Pending => "等待解析",
        QueueItemState.Parsing => "正在解析…",
        QueueItemState.Parsed => "已解析，等待下载",
        QueueItemState.Downloading => $"下载中 {ProgressText}",
        QueueItemState.Completed => "已完成",
        QueueItemState.Failed => string.IsNullOrWhiteSpace(Error) ? "失败" : $"失败：{Error}",
        QueueItemState.Cancelled => "已取消",
        _ => string.Empty,
    };

    /// <summary>True when a valid manual selection exists for the given download kind.</summary>
    public bool HasValidManualSelection(DownloadKind kind) => kind switch
    {
        DownloadKind.VideoOnly => _selectedVideo is not null,
        DownloadKind.AudioOnly => _selectedAudio is not null,
        _ => _selectedVideo is not null && (_selectedVideo.HasAudio || _selectedAudio is not null),
    };

    public void MarkParsing() => State = QueueItemState.Parsing;

    /// <summary>Stores parsed metadata and marks the item ready to download.</summary>
    public void ApplyInfo(
        string title,
        string? thumbnailUrl,
        ImageSource? thumbnail,
        IEnumerable<VideoSource> videos,
        IEnumerable<AudioSource> audios)
    {
        if (!string.IsNullOrWhiteSpace(title))
            Title = title;

        ThumbnailUrl = thumbnailUrl;
        Thumbnail = thumbnail;
        Error = null;

        VideoSources.Clear();
        foreach (var video in videos)
            VideoSources.Add(video);

        AudioSources.Clear();
        foreach (var audio in audios)
            AudioSources.Add(audio);

        State = QueueItemState.Parsed;
    }

    public void MarkDownloading()
    {
        Progress = 0;
        Error = null;
        State = QueueItemState.Downloading;
    }

    public void ReportProgress(double percent) => Progress = percent;

    public void MarkCompleted(string? outputPath)
    {
        OutputPath = outputPath;
        Progress = 100;
        State = QueueItemState.Completed;
    }

    public void MarkFailed(string? error)
    {
        Error = error;
        State = QueueItemState.Failed;
    }

    public void MarkCancelled() => State = QueueItemState.Cancelled;

    /// <summary>Resets the item back to pending so it can be parsed/downloaded again.</summary>
    public void Reset()
    {
        Error = null;
        OutputPath = null;
        Progress = 0;
        State = QueueItemState.Pending;
    }

    private void NotifySelectionChanged()
    {
        OnPropertyChanged(nameof(SelectionInfo));
        OnPropertyChanged(nameof(HasSelection));
    }

    private void NotifyStateDependents()
    {
        OnPropertyChanged(nameof(IsPending));
        OnPropertyChanged(nameof(IsParsing));
        OnPropertyChanged(nameof(IsParsed));
        OnPropertyChanged(nameof(IsDownloading));
        OnPropertyChanged(nameof(IsCompleted));
        OnPropertyChanged(nameof(IsFailed));
        OnPropertyChanged(nameof(IsCancelled));
        OnPropertyChanged(nameof(ShowProgress));
        OnPropertyChanged(nameof(CanRetry));
        OnPropertyChanged(nameof(StatusText));
    }
}