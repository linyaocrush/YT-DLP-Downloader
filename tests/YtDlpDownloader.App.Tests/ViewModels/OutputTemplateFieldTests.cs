using YtDlpDownloader.App.ViewModels;

namespace YtDlpDownloader.App.Tests.ViewModels;

public class OutputTemplateFieldTests
{
    [Fact]
    public void Constructor_SetsTokenLabelAndSelection()
    {
        var field = new OutputTemplateField("%(title)s", "标题", isSelected: true);

        Assert.Equal("%(title)s", field.Token);
        Assert.Equal("标题", field.Label);
        Assert.True(field.IsSelected);
    }

    [Fact]
    public void IsSelected_DefaultsToFalse()
        => Assert.False(new OutputTemplateField("%(id)s", "ID").IsSelected);

    [Fact]
    public void IsSelected_Change_RaisesPropertyChanged()
    {
        var field = new OutputTemplateField("%(id)s", "ID");
        string? raised = null;
        field.PropertyChanged += (_, e) => raised = e.PropertyName;

        field.IsSelected = true;

        Assert.True(field.IsSelected);
        Assert.Equal(nameof(OutputTemplateField.IsSelected), raised);
    }

    [Fact]
    public void IsSelected_SameValue_DoesNotRaise()
    {
        var field = new OutputTemplateField("%(id)s", "ID");
        var raised = false;
        field.PropertyChanged += (_, _) => raised = true;

        field.IsSelected = false;

        Assert.False(raised);
    }
}
