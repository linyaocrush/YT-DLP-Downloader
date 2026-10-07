using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using Microsoft.Win32;
using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.App.Theming;

/// <summary>
/// Swaps the application colour palette at runtime and keeps window chrome in sync.
/// Only one palette dictionary (<c>Styles/LightTheme.xaml</c> or <c>Styles/DarkTheme.xaml</c>)
/// is merged into <see cref="Application.Resources"/> at a time; the styles in
/// <c>Styles/Win11Theme.xaml</c> reference it through <c>DynamicResource</c>.
/// </summary>
public static class ThemeManager
{
    private const string LightPaletteUri = "pack://application:,,,/Styles/LightTheme.xaml";
    private const string DarkPaletteUri = "pack://application:,,,/Styles/DarkTheme.xaml";

    // DwmSetWindowAttribute attribute ids for the immersive dark title bar.
    private const int DwmwaUseImmersiveDarkMode = 20;
    private const int DwmwaUseImmersiveDarkModeLegacy = 19;

    private static ResourceDictionary? _palette;
    private static AppTheme _mode = AppTheme.System;
    private static bool? _appliedDark;
    private static bool _systemListenerAttached;

    /// <summary>Applies the requested scheme; a no-op when the resolved scheme is unchanged.</summary>
    public static void Apply(AppTheme mode)
    {
        _mode = mode;
        AttachSystemListener();

        var app = Application.Current;
        if (app is null)
            return;

        var dark = IsDark(mode);
        if (_appliedDark == dark)
        {
            ApplyWindowChrome();
            return;
        }

        var palette = new ResourceDictionary
        {
            Source = new Uri(dark ? DarkPaletteUri : LightPaletteUri, UriKind.Absolute),
        };

        var dictionaries = app.Resources.MergedDictionaries;
        if (_palette is not null)
        {
            var index = dictionaries.IndexOf(_palette);
            if (index >= 0)
                dictionaries.RemoveAt(index);
        }

        dictionaries.Insert(0, palette);
        _palette = palette;
        _appliedDark = dark;

        ApplyWindowChrome();
    }

    /// <summary>Applies the current scheme's title-bar colour to every open window.</summary>
    public static void ApplyWindowChrome()
    {
        var app = Application.Current;
        if (app is null)
            return;

        foreach (Window window in app.Windows)
            ApplyWindowChrome(window);
    }

    /// <summary>Applies the current scheme's title-bar colour to a single window.</summary>
    public static void ApplyWindowChrome(Window window)
    {
        if (window is null)
            return;

        var handle = new WindowInteropHelper(window).Handle;
        if (handle == IntPtr.Zero)
            return;

        var useDark = _appliedDark == true ? 1 : 0;
        if (DwmSetWindowAttribute(handle, DwmwaUseImmersiveDarkMode, ref useDark, sizeof(int)) != 0)
            DwmSetWindowAttribute(handle, DwmwaUseImmersiveDarkModeLegacy, ref useDark, sizeof(int));
    }

    private static bool IsDark(AppTheme mode) => mode switch
    {
        AppTheme.Dark => true,
        AppTheme.Light => false,
        _ => IsSystemDark(),
    };

    private static bool IsSystemDark()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(
                @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            return key?.GetValue("AppsUseLightTheme") is int light && light == 0;
        }
        catch
        {
            // Missing key or access denied: fall back to the light palette.
            return false;
        }
    }

    private static void AttachSystemListener()
    {
        if (_systemListenerAttached)
            return;

        _systemListenerAttached = true;
        SystemEvents.UserPreferenceChanged += OnUserPreferenceChanged;
    }

    private static void OnUserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
    {
        var app = Application.Current;
        if (app is null || _mode != AppTheme.System)
            return;

        app.Dispatcher.Invoke(() =>
        {
            if (_mode == AppTheme.System)
                Apply(AppTheme.System);
        });
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int value, int size);
}
