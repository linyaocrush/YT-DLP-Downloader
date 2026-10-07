using System.Globalization;
using System.Windows;
using YtDlpDownloader.App.Converters;

namespace YtDlpDownloader.App.Tests.Converters;

public class BoolToGridLengthConverterTests
{
    private readonly BoolToGridLengthConverter _converter = new();

    private object Convert(object value, object? parameter = null)
        => _converter.Convert(value, typeof(GridLength), parameter!, CultureInfo.InvariantCulture);

    [Theory]
    [InlineData(false)]
    [InlineData(null)]
    [InlineData("true")]
    public void Convert_NonTrueValue_ReturnsZeroLength(object? value)
    {
        var result = Assert.IsType<GridLength>(Convert(value!));

        Assert.Equal(0, result.Value);
    }

    [Fact]
    public void Convert_TrueWithPixelParameter_ReturnsPixelLength()
    {
        var result = Assert.IsType<GridLength>(Convert(true, "200"));

        Assert.Equal(200, result.Value);
        Assert.Equal(GridUnitType.Pixel, result.GridUnitType);
    }

    [Fact]
    public void Convert_TrueWithoutParameter_ReturnsStarLength()
    {
        var result = Assert.IsType<GridLength>(Convert(true));

        Assert.Equal(1, result.Value);
        Assert.Equal(GridUnitType.Star, result.GridUnitType);
    }

    [Fact]
    public void Convert_TrueWithInvalidParameter_ReturnsStarLength()
    {
        var result = Assert.IsType<GridLength>(Convert(true, "not-a-number"));

        Assert.Equal(GridUnitType.Star, result.GridUnitType);
    }

    [Fact]
    public void ConvertBack_Throws()
        => Assert.Throws<NotSupportedException>(() =>
            _converter.ConvertBack(new GridLength(1, GridUnitType.Star), typeof(bool), null!, CultureInfo.InvariantCulture));
}
