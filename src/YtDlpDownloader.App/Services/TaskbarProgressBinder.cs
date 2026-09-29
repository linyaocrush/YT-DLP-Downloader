using System.ComponentModel;
using System.Windows;
using System.Windows.Shell;
using System.Windows.Threading;
using YtDlpDownloader.App.ViewModels;

namespace YtDlpDownloader.App.Services;

/// <summary>Mirrors <see cref="DownloadViewModel"/> state onto the window's taskbar button.</summary>
public sealed class TaskbarProgressBinder : IDisposable
{
    private static readonly TimeSpan CompletionDisplayDuration = TimeSpan.FromSeconds(3);

    private readonly DownloadViewModel _download;
    private readonly TaskbarItemInfo _taskbar;
    private readonly DispatcherTimer _resetTimer;

    public TaskbarProgressBinder(Window window, DownloadViewModel download)
    {
        _download = download;
        _taskbar = window.TaskbarItemInfo ??= new TaskbarItemInfo();

        _resetTimer = new DispatcherTimer { Interval = CompletionDisplayDuration };
        _resetTimer.Tick += (_, _) =>
        {
            _resetTimer.Stop();
            _taskbar.ProgressState = TaskbarItemProgressState.None;
        };

        _download.PropertyChanged += OnDownloadPropertyChanged;
        _download.DownloadFinished += OnDownloadFinished;
    }

    private void OnDownloadPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        switch (e.PropertyName)
        {
            case nameof(DownloadViewModel.Progress):
                if (_taskbar.ProgressState == TaskbarItemProgressState.Normal)
                    _taskbar.ProgressValue = _download.Progress / 100d;
                break;

            case nameof(DownloadViewModel.IsProgressVisible):
                if (_download.IsProgressVisible)
                {
                    _resetTimer.Stop();
                    _taskbar.ProgressState = TaskbarItemProgressState.Normal;
                    _taskbar.ProgressValue = _download.Progress / 100d;
                }
                break;
        }
    }

    private void OnDownloadFinished(object? sender, DownloadFinishedEventArgs e)
    {
        if (e.Cancelled)
        {
            _taskbar.ProgressState = TaskbarItemProgressState.None;
            return;
        }

        if (e.Success)
        {
            _taskbar.ProgressState = TaskbarItemProgressState.Normal;
            _taskbar.ProgressValue = 1;
            _resetTimer.Stop();
            _resetTimer.Start();
        }
        else
        {
            _taskbar.ProgressState = TaskbarItemProgressState.Error;
            _taskbar.ProgressValue = 1;
        }
    }

    public void Dispose()
    {
        _resetTimer.Stop();
        _download.PropertyChanged -= OnDownloadPropertyChanged;
        _download.DownloadFinished -= OnDownloadFinished;
    }
}