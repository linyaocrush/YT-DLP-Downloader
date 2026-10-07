using System.Drawing;
using System.IO;
using System.Windows;
using YtDlpDownloader.App.ViewModels;
using Forms = System.Windows.Forms;

namespace YtDlpDownloader.App.Services;

/// <summary>Shows a Windows notification (tray balloon) when a download finishes.</summary>
public sealed class DownloadNotifier : IDisposable
{
    private const int BalloonTimeoutMs = 5000;
    private const int MaxBodyLength = 250;

    private readonly Window _window;
    private readonly DownloadViewModel _download;
    private readonly Forms.NotifyIcon _icon;

    public DownloadNotifier(Window window, DownloadViewModel download)
    {
        _window = window;
        _download = download;

        _icon = new Forms.NotifyIcon
        {
            Icon = SystemIcons.Application,
            Text = "YtDlp 下载器",
            Visible = true,
        };
        _icon.BalloonTipClicked += (_, _) => ActivateWindow();
        _icon.DoubleClick += (_, _) => ActivateWindow();

        _download.DownloadFinished += OnDownloadFinished;
    }

    private void OnDownloadFinished(object? sender, DownloadFinishedEventArgs e)
    {
        if (e.Cancelled)
            return;

        if (e.Success)
        {
            var body = string.IsNullOrWhiteSpace(e.Title)
                ? SavedPathText(e.OutputPath)
                : $"《{e.Title}》已保存";
            _icon.ShowBalloonTip(BalloonTimeoutMs, "下载完成", Truncate(body), Forms.ToolTipIcon.Info);
        }
        else
        {
            _icon.ShowBalloonTip(BalloonTimeoutMs, "下载失败", Truncate(e.Error ?? "未知错误"), Forms.ToolTipIcon.Error);
        }
    }

    private void ActivateWindow()
    {
        if (_window.WindowState == WindowState.Minimized)
            _window.WindowState = WindowState.Normal;

        _window.Activate();
        _window.Topmost = true;
        _window.Topmost = false;
    }

    private static string SavedPathText(string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
            return "文件已保存";

        var name = Path.GetFileName(path);
        return string.IsNullOrEmpty(name) ? $"已保存到 {path}" : $"已保存到 {name}";
    }

    private static string Truncate(string text)
        => text.Length <= MaxBodyLength ? text : text[..(MaxBodyLength - 3)] + "...";

    public void Dispose()
    {
        _download.DownloadFinished -= OnDownloadFinished;
        _icon.Visible = false;
        _icon.Dispose();
    }
}
