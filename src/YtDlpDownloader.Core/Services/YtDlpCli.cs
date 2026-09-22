using System.Text.RegularExpressions;
using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.Core.Services;

/// <summary>Raised when yt-dlp reports a non-zero exit code while parsing metadata.</summary>
public sealed class YtDlpException : Exception
{
    public YtDlpException(string message) : base(message) { }
}

public interface IYtDlpCli
{
    string? ResolveExecutable();

    Task<string?> GetVersionAsync(CancellationToken cancellationToken = default);

    Task<MediaInfo> GetMediaInfoAsync(
        string url,
        string? cookieFile = null,
        CancellationToken cancellationToken = default);

    Task<DownloadOutcome> DownloadAsync(
        string url,
        string formatExpression,
        string outputDirectory,
        string? cookieFile = null,
        int concurrentFragments = 1,
        string? outputTemplate = null,
        IProgress<DownloadUpdate>? progress = null,
        CancellationToken cancellationToken = default);
}

public sealed record DownloadOutcome(
    bool Success,
    bool Cancelled,
    string? OutputPath,
    string? Error);

/// <summary>One incremental update reported while a download runs.</summary>
public sealed record DownloadUpdate(string? Line, double? Percent, string? OutputPath);

public sealed class YtDlpCli : IYtDlpCli
{
    private const string NotConfiguredMessage =
        "未找到 yt-dlp。请到“设置”页选择 yt-dlp.exe 的路径。";

    private static readonly Regex PercentRegex = new(
        @"\[download\]\s+(\d+(?:\.\d+)?)%",
        RegexOptions.Compiled);

    private static readonly Regex DestinationRegex = new(
        @"\[download\]\s+Destination:\s+(.+?)\s*$",
        RegexOptions.Compiled);

    private static readonly Regex MergeRegex = new(
        @"Merging formats into\s+\""(.+?)\""\s*$",
        RegexOptions.Compiled);

    private readonly IProcessRunner _runner;
    private readonly IYtDlpPathService _paths;
    private readonly IFfmpegPathService _ffmpegPaths;
    private readonly IMediaInfoParser _parser;

    public YtDlpCli(
        IProcessRunner runner,
        IYtDlpPathService paths,
        IFfmpegPathService ffmpegPaths,
        IMediaInfoParser parser)
    {
        _runner = runner;
        _paths = paths;
        _ffmpegPaths = ffmpegPaths;
        _parser = parser;
    }

    public string? ResolveExecutable() => _paths.Resolve();

    public async Task<string?> GetVersionAsync(CancellationToken cancellationToken = default)
    {
        var exe = _paths.Resolve();
        if (exe is null)
            return null;

        var result = await _runner.RunAsync(
            exe, new[] { "--version" }, cancellationToken: cancellationToken);

        return result.ExitCode == 0 ? result.StandardOutput.Trim() : null;
    }

    public async Task<MediaInfo> GetMediaInfoAsync(
        string url,
        string? cookieFile = null,
        CancellationToken cancellationToken = default)
    {
        var exe = _paths.Resolve() ?? throw new YtDlpException(NotConfiguredMessage);

        var args = new List<string> { "--encoding", "utf-8", "--skip-download", "--no-playlist", "--no-warnings", "--dump-single-json" };
        AddCookieArguments(args, cookieFile);
        args.Add(url);

        var result = await _runner.RunAsync(exe, args, cancellationToken: cancellationToken);

        if (result.Cancelled)
            throw new OperationCanceledException(cancellationToken);

        if (result.ExitCode != 0)
            throw new YtDlpException(LastMeaningful(result.StandardError, result.StandardOutput));

        try
        {
            return _parser.Parse(result.StandardOutput);
        }
        catch (Exception ex) when (ex is not OperationCanceledException)
        {
            throw new YtDlpException($"解析视频信息失败：{ex.Message}");
        }
    }

    public async Task<DownloadOutcome> DownloadAsync(
        string url,
        string formatExpression,
        string outputDirectory,
        string? cookieFile = null,
        int concurrentFragments = 1,
        string? outputTemplate = null,
        IProgress<DownloadUpdate>? progress = null,
        CancellationToken cancellationToken = default)
    {
        var exe = _paths.Resolve();
        if (exe is null)
            return new DownloadOutcome(false, false, null, NotConfiguredMessage);

        try
        {
            Directory.CreateDirectory(outputDirectory);
        }
        catch (Exception ex)
        {
            return new DownloadOutcome(false, false, null, $"无法创建输出目录：{ex.Message}");
        }

        var nameTemplate = string.IsNullOrWhiteSpace(outputTemplate)
            ? "%(title)s.%(ext)s"
            : outputTemplate.Trim();
        var template = Path.Combine(outputDirectory, nameTemplate);
        var args = new List<string>
        {
            "--encoding", "utf-8",
            "--no-playlist",
            "--newline",
            "--no-warnings",
            "-f", formatExpression,
            "-o", template,
        };
        AddCookieArguments(args, cookieFile);

        // Only pass an explicit location when ffmpeg is not already on PATH (yt-dlp finds it there).
        if (_ffmpegPaths.GetExplicitPathForYtDlp() is { } ffmpeg)
        {
            args.Add("--ffmpeg-location");
            args.Add(ffmpeg);
        }

        if (concurrentFragments > 1)
        {
            args.Add("--concurrent-fragments");
            args.Add(concurrentFragments.ToString());
        }

        args.Add(url);

        string? lastDestination = null;
        string? finalPath = null;
        void OnLine(string line)
        {
            var trimmed = line.Trim('\r');
            if (progress is null)
                return;

            var percent = PercentRegex.Match(trimmed);
            if (percent.Success
                && double.TryParse(percent.Groups[1].Value, out var value))
            {
                progress.Report(new DownloadUpdate(null, Math.Clamp(value, 0, 100), null));
                return;
            }

            var merge = MergeRegex.Match(trimmed);
            if (merge.Success)
            {
                var path = merge.Groups[1].Value.Trim();
                lastDestination = path;
                finalPath = path;
                progress.Report(new DownloadUpdate(null, null, path));
                return;
            }

            var destination = DestinationRegex.Match(trimmed);
            if (destination.Success)
            {
                lastDestination = destination.Groups[1].Value.Trim();
                progress.Report(new DownloadUpdate(null, null, lastDestination));
                return;
            }

            progress.Report(new DownloadUpdate(trimmed, null, null));
        }

        try
        {
            var result = await _runner.RunAsync(
                exe, args, OnLine, OnLine, cancellationToken);

            if (result.Cancelled)
                return new DownloadOutcome(false, true, null, null);

            if (result.ExitCode != 0)
                return new DownloadOutcome(
                    false, false, null, LastMeaningful(result.StandardError, result.StandardOutput));

            return new DownloadOutcome(true, false, finalPath ?? lastDestination, null);
        }
        catch (OperationCanceledException)
        {
            return new DownloadOutcome(false, true, null, null);
        }
    }

    private static void AddCookieArguments(ICollection<string> args, string? cookieFile)
    {
        if (string.IsNullOrWhiteSpace(cookieFile))
            return;

        args.Add("--cookies");
        args.Add(cookieFile.Trim());
    }

    private static string LastMeaningful(string stderr, string stdout)
    {
        var lines = (stderr + "\n" + stdout)
            .Split('\n')
            .Select(line => line.Trim())
            .Where(line => line.Length > 0)
            .ToArray();

        return lines.Length == 0
            ? "未知错误（yt-dlp 非零退出码）"
            : string.Join(" | ", lines.TakeLast(3));
    }
}
