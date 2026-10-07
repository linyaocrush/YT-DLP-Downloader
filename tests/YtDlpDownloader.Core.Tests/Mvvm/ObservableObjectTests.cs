using System.ComponentModel;
using YtDlpDownloader.Core.Mvvm;

namespace YtDlpDownloader.Core.Tests.Mvvm;

public class ObservableObjectTests
{
    private sealed class TestObject : ObservableObject
    {
        private int _value;

        public int Value
        {
            get => _value;
            set => SetProperty(ref _value, value);
        }

        public void Rename(string name) => OnPropertyChanged(name);
    }

    [Fact]
    public void SetProperty_NewValue_UpdatesAndRaisesPropertyChanged()
    {
        var obj = new TestObject();
        string? raised = null;
        obj.PropertyChanged += (_, e) => raised = e.PropertyName;

        obj.Value = 5;

        Assert.Equal(5, obj.Value);
        Assert.Equal(nameof(TestObject.Value), raised);
    }

    [Fact]
    public void SetProperty_SameValue_DoesNotRaiseOrChange()
    {
        var obj = new TestObject { Value = 3 };
        var raised = false;
        obj.PropertyChanged += (_, _) => raised = true;

        obj.Value = 3;

        Assert.False(raised);
    }

    [Fact]
    public void OnPropertyChanged_ExplicitName_Raises()
    {
        var obj = new TestObject();
        var args = new List<PropertyChangedEventArgs>();
        obj.PropertyChanged += (_, e) => args.Add(e);

        obj.Rename("Custom");

        Assert.Single(args);
        Assert.Equal("Custom", args[0].PropertyName);
    }
}
