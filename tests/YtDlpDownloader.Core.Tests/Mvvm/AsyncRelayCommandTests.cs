using YtDlpDownloader.Core.Mvvm;

namespace YtDlpDownloader.Core.Tests.Mvvm;

public class AsyncRelayCommandTests
{
    [Fact]
    public void Constructor_NullExecute_Throws()
        => Assert.Throws<ArgumentNullException>(() => new AsyncRelayCommand(null!));

    [Fact]
    public async Task Execute_RunsDelegate()
    {
        var completed = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var command = new AsyncRelayCommand(() =>
        {
            completed.SetResult();
            return Task.CompletedTask;
        });

        command.Execute(null);
        await completed.Task.WaitAsync(TimeSpan.FromSeconds(5));

        Assert.True(true);
    }

    [Fact]
    public async Task Execute_WhileRunning_CanExecuteIsFalseThenTrue()
    {
        var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var started = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var command = new AsyncRelayCommand(async () =>
        {
            started.SetResult();
            await release.Task;
        });

        Assert.True(command.CanExecute(null));
        command.Execute(null);
        await started.Task.WaitAsync(TimeSpan.FromSeconds(5));

        Assert.False(command.CanExecute(null));

        release.SetResult();
        await WaitUntilAsync(() => command.CanExecute(null));
        Assert.True(command.CanExecute(null));
    }

    [Fact]
    public async Task Execute_ThrowingDelegate_InvokesErrorHandler()
    {
        var caught = new TaskCompletionSource<Exception>(TaskCreationOptions.RunContinuationsAsynchronously);
        var command = new AsyncRelayCommand(
            () => throw new InvalidOperationException("boom"),
            onError: ex => caught.SetResult(ex));

        command.Execute(null);
        var exception = await caught.Task.WaitAsync(TimeSpan.FromSeconds(5));

        Assert.IsType<InvalidOperationException>(exception);
        Assert.Equal("boom", exception.Message);
    }

    [Fact]
    public async Task Execute_ThrowingDelegate_WithoutHandler_DoesNotThrow()
    {
        var release = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var command = new AsyncRelayCommand(() =>
        {
            release.SetResult();
            throw new InvalidOperationException("boom");
        });

        command.Execute(null);
        await release.Task.WaitAsync(TimeSpan.FromSeconds(5));
        await WaitUntilAsync(() => command.CanExecute(null));

        Assert.True(command.CanExecute(null));
    }

    [Fact]
    public void Execute_WhenCanExecuteFalse_DoesNotRun()
    {
        var invoked = false;
        var command = new AsyncRelayCommand(
            () => { invoked = true; return Task.CompletedTask; },
            canExecute: () => false);

        command.Execute(null);

        Assert.False(invoked);
    }

    [Fact]
    public void NotifyCanExecuteChanged_RaisesEvent()
    {
        var command = new AsyncRelayCommand(() => Task.CompletedTask);
        var raised = false;
        command.CanExecuteChanged += (_, _) => raised = true;

        command.NotifyCanExecuteChanged();

        Assert.True(raised);
    }

    private static async Task WaitUntilAsync(Func<bool> condition)
    {
        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        while (!condition())
        {
            await Task.Delay(10, cts.Token);
        }
    }
}
