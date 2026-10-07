using System;
using System.Windows;
using System.Windows.Media;

namespace YtDlpDownloader.App.Views;

public partial class MainWindow : Window
{
    private const double DesignWidth = 1160d;
    private const double MinScale = 0.65d;
    private const double MaxScale = 1d;

    private readonly ScaleTransform _uiScale = new(1d, 1d);

    public MainWindow()
    {
        InitializeComponent();
        RootTabs.LayoutTransform = _uiScale;
        SizeChanged += OnWindowSizeChanged;
    }

    private void OnWindowSizeChanged(object sender, SizeChangedEventArgs e)
    {
        var scale = Math.Clamp(ActualWidth / DesignWidth, MinScale, MaxScale);
        if (Math.Abs(scale - _uiScale.ScaleX) < 0.001d)
            return;

        _uiScale.ScaleX = scale;
        _uiScale.ScaleY = scale;
    }
}
