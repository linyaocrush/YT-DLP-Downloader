using YtDlpDownloader.Core.Mvvm;

namespace YtDlpDownloader.App.ViewModels;

public sealed class MainViewModel : ObservableObject
{
    public MainViewModel(DownloadViewModel download, SettingsViewModel settings)
    {
        Download = download;
        Settings = settings;
    }

    public string WindowTitle => "YtDlp 下载器";

    public DownloadViewModel Download { get; }
    public SettingsViewModel Settings { get; }
}
