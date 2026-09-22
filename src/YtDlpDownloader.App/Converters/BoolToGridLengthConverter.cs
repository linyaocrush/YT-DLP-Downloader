using System;
using System.Globalization;
using System.Windows;
using System.Windows.Data;

namespace YtDlpDownloader.App.Converters;

public sealed class BoolToGridLengthConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, CultureInfo culture)
    {
        if (value is not true)
            return new GridLength(0);

        if (parameter is string text
            && double.TryParse(text, NumberStyles.Any, CultureInfo.InvariantCulture, out var pixels))
            return new GridLength(pixels);

        return new GridLength(1, GridUnitType.Star);
    }

    public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
