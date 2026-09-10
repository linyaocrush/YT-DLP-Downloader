using System.Diagnostics;
using System.Text;

namespace YtDlpDownloader.Core.Services;

public sealed record ProcessResult(
    bool Cancelled,
    int ExitCode,
    string StandardOutput,
    string StandardError);

public interface IProcessRunner
{
    /// <summary>
    /// Starts <paramref name="fileName"/> with UTF-8 redirected stdout/stderr, streaming each line
    /// to the supplied callbacks (invoked on background threads). Cancellation kills the process tree.
    /// </summary>
    Task<ProcessResult> RunAsync(
        string fileName,
        IReadOnlyList<string> arguments,
        Action<string>? onStdOutLine = null,
        Action<string>? onStdErrLine = null,
        CancellationToken cancellationToken = default);
}

public sealed class ProcessRunner : IProcessRunner
{
    public async Task<ProcessResult> RunAsync(
        string fileName,
        IReadOnlyList<string> arguments,
        Action<string>? onStdOutLine = null,
        Action<string>? onStdErrLine = null,
        CancellationToken cancellationToken = default)
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = fileName,
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true,
            StandardOutputEncoding = Encoding.UTF8,
            StandardErrorEncoding = Encoding.UTF8,
        };

        foreach (var argument in arguments)
            startInfo.ArgumentList.Add(argument);

        using var process = new Process
        {
            StartInfo = startInfo,
            EnableRaisingEvents = true,
        };

        var output = new StringBuilder();
        var error = new StringBuilder();
        var stdoutDone = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);
        var stderrDone = new TaskCompletionSource<bool>(TaskCreationOptions.RunContinuationsAsynchronously);

        process.OutputDataReceived += (_, e) =>
        {
            if (e.Data is null)
            {
                stdoutDone.TrySetResult(true);
                return;
            }

            lock (output)
                output.AppendLine(e.Data);
            onStdOutLine?.Invoke(e.Data);
        };

        process.ErrorDataReceived += (_, e) =>
        {
            if (e.Data is null)
            {
                stderrDone.TrySetResult(true);
                return;
            }

            lock (error)
                error.AppendLine(e.Data);
            onStdErrLine?.Invoke(e.Data);
        };

        using var cancelRegistration = cancellationToken.Register(() =>
        {
            try
            {
                if (!process.HasExited)
                    process.Kill(entireProcessTree: true);
            }
            catch
            {
                // Process already exited between the check and the kill.
            }
        });

        if (!process.Start())
            throw new InvalidOperationException($"无法启动进程：{fileName}");

        process.BeginOutputReadLine();
        process.BeginErrorReadLine();

        try
        {
            await process.WaitForExitAsync(cancellationToken);
        }
        catch (OperationCanceledException)
        {
            // Cancellation handler already killed the tree; give it a moment to die.
            process.WaitForExit(3000);
        }

        process.WaitForExit();
        await Task.WhenAll(stdoutDone.Task, stderrDone.Task).WaitAsync(TimeSpan.FromSeconds(5));

        return new ProcessResult(
            cancellationToken.IsCancellationRequested,
            process.ExitCode,
            output.ToString(),
            error.ToString());
    }
}
