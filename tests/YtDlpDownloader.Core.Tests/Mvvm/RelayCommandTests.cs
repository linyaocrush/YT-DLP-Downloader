using YtDlpDownloader.Core.Mvvm;

namespace YtDlpDownloader.Core.Tests.Mvvm;

public class RelayCommandTests
{
    [Fact]
    public void Constructor_NullExecute_Throws()
        => Assert.Throws<ArgumentNullException>(() => new RelayCommand(null!));

    [Fact]
    public void Execute_InvokesDelegate()
    {
        var count = 0;
        var command = new RelayCommand(() => count++);

        command.Execute(null);

        Assert.Equal(1, count);
    }

    [Fact]
    public void CanExecute_WithoutPredicate_IsTrue()
        => Assert.True(new RelayCommand(() => { }).CanExecute(null));

    [Fact]
    public void CanExecute_UsesPredicate()
    {
        var allowed = false;
        var command = new RelayCommand(() => { }, () => allowed);

        Assert.False(command.CanExecute(null));
        allowed = true;
        Assert.True(command.CanExecute(null));
    }

    [Fact]
    public void NotifyCanExecuteChanged_RaisesEvent()
    {
        var command = new RelayCommand(() => { });
        var raised = false;
        command.CanExecuteChanged += (_, _) => raised = true;

        command.NotifyCanExecuteChanged();

        Assert.True(raised);
    }

    [Fact]
    public void GenericConstructor_NullExecute_Throws()
        => Assert.Throws<ArgumentNullException>(() => new RelayCommand<int>(null!));

    [Fact]
    public void GenericExecute_ForwardsTypedParameter()
    {
        var received = 0;
        var command = new RelayCommand<int>(value => received = value);

        command.Execute(42);

        Assert.Equal(42, received);
    }

    [Fact]
    public void GenericExecute_WrongParameterType_DoesNotInvoke()
    {
        var invoked = false;
        var command = new RelayCommand<int>(_ => invoked = true);

        command.Execute("not an int");

        Assert.False(invoked);
    }

    [Fact]
    public void GenericCanExecute_TypedParameter_UsesPredicate()
    {
        var command = new RelayCommand<int>(_ => { }, value => value > 10);

        Assert.True(command.CanExecute(11));
        Assert.False(command.CanExecute(5));
    }

    [Fact]
    public void GenericCanExecute_WrongType_IsFalse()
    {
        var command = new RelayCommand<int>(_ => { });

        Assert.False(command.CanExecute("text"));
    }

    [Fact]
    public void GenericCanExecute_Null_TrueOnlyWhenNoPredicate()
    {
        Assert.True(new RelayCommand<string>(_ => { }).CanExecute(null));
        Assert.False(new RelayCommand<string>(_ => { }, _ => true).CanExecute(null));
    }
}
