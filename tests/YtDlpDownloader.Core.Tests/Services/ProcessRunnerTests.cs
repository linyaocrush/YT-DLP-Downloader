using System.ComponentModel;
using YtDlpDownloader.Core.Services;

namespace YtDlpDownloader.Core.Tests.Services;

public class ProcessRunnerTests
{
    private static string SystemExecutable(string name)
        => Path.Combine(Environment.SystemDirectory, name);

    [Fact]
    public async Task RunAsync_MissingExecutable_Throws()
    {
        using var temp = new TempDirectory();
        var runner = new ProcessRunner();

        await Assert.ThrowsAsync<Win32Exception>(() =>
            runner.RunAsync(Path.Combine(temp.Path, "missing.exe"), Array.Empty<string>()));
    }

    [Fact]
    public async Task RunAsync_Echo_ReturnsStdoutAndExitCode()
    {
        var runner = new ProcessRunner();
        var lines = new List<string>();

        var result = await runner.RunAsync(
            SystemExecutable("cmd.exe"),
            new[] { "/c", "echo", "hello-world" },
            onStdOutLine: lines.Add);

        Assert.False(result.Cancelled);
        Assert.Equal(0, result.ExitCode);
        Assert.Contains("hello-world", result.StandardOutput);
        Assert.Contains("hello-world", lines);
    }

    [Fact]
    public async Task RunAsync_NonZeroExit_CapturesStderr()
    {
        var runner = new ProcessRunner();

        var result = await runner.RunAsync(
            SystemExecutable("cmd.exe"),
            new[] { "/c", "echo", "oops", "1>&2", "&", "exit", "3" });

        Assert.Equal(3, result.ExitCode);
        Assert.Contains("oops", result.StandardError);
    }

    [Fact]
    public async Task RunAsync_Cancelled_KillsProcessAndReportsCancellation()
    {
        var runner = new ProcessRunner();
        using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(500));

        var result = await runner.RunAsync(
            SystemExecutable("cmd.exe"),
            new[] { "/c", "ping", "-n", "8", "127.0.0.1" },
            cancellationToken: cts.Token);

        Assert.True(result.Cancelled);
    }
}
