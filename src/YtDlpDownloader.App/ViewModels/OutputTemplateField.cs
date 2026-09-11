using YtDlpDownloader.Core.Mvvm;

namespace YtDlpDownloader.App.ViewModels;

/// <summary>A toggleable yt-dlp output-template field rendered as a chip button.</summary>
public sealed class OutputTemplateField : ObservableObject
{
    private bool _isSelected;

    public OutputTemplateField(string token, string label, bool isSelected = false)
    {
        Token = token;
        Label = label;
        _isSelected = isSelected;
    }

    /// <summary>The literal yt-dlp template token, e.g. <c>%(title)s</c>.</summary>
    public string Token { get; }

    /// <summary>Localized display text shown on the chip.</summary>
    public string Label { get; }

    public bool IsSelected
    {
        get => _isSelected;
        set => SetProperty(ref _isSelected, value);
    }
}
