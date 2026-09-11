using System.Text.Json;
using System.Text.Json.Serialization;
using YtDlpDownloader.Core.Models;

namespace YtDlpDownloader.Core.Services;

public interface ISettingsService
{
    AppSettings Settings { get; }

    /// <summary>Raised after a successful save.</summary>
    event EventHandler? Changed;

    void Save();
}

public sealed class SettingsService : ISettingsService
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter() },
    };

    private readonly string _filePath;
    private readonly AppSettings _settings;

    public event EventHandler? Changed;

    public SettingsService(string? directory = null)
    {
        directory ??= Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "YtDlpDownloader");

        Directory.CreateDirectory(directory);
        _filePath = Path.Combine(directory, "settings.json");
        _settings = Load();
    }

    public AppSettings Settings => _settings;

    private AppSettings Load()
    {
        try
        {
            if (File.Exists(_filePath))
            {
                var deserialized = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(_filePath));
                if (deserialized is not null)
                    return deserialized;
            }
        }
        catch
        {
            // Corrupted settings are ignored and replaced by defaults.
        }

        return new AppSettings();
    }

    public void Save()
    {
        try
        {
            File.WriteAllText(_filePath, JsonSerializer.Serialize(_settings, JsonOptions));
        }
        catch
        {
            // Never crash the UI because a settings file could not be written.
        }

        Changed?.Invoke(this, EventArgs.Empty);
    }
}
