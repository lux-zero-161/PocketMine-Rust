using PocketMine.Command;
using PocketMine.Scheduler;
using PocketMine.Server;
using PocketMine.Utils;

namespace PocketMine.Plugin;

/// <summary>
/// Declares a plugin. Mirrors the fields of PocketMine-MP's <c>plugin.yml</c>.
/// </summary>
[AttributeUsage(AttributeTargets.Class, Inherited = false, AllowMultiple = false)]
public sealed class PluginAttribute : Attribute
{
    public string Name { get; }
    public string Version { get; }
    public string Author { get; set; } = "Unknown";
    public string Description { get; set; } = "";
    public string Api { get; set; } = "1.0.0.0";
    public string[] Depends { get; set; } = Array.Empty<string>();
    public string[] LoadBefore { get; set; } = Array.Empty<string>();
    public string[] SoftDepends { get; set; } = Array.Empty<string>();

    public PluginAttribute(string name, string version)
    {
        Name = name;
        Version = version;
    }
}

/// <summary>
/// Base class for every C# plugin. Lifecycle matches PocketMine-MP:
/// <c>OnLoad</c> is called once after construction, <c>OnEnable</c> when the
/// server starts (or after <c>/reload</c>) and <c>OnDisable</c> on shutdown.
/// </summary>
public abstract class PluginBase
{
    public string Name { get; internal set; } = string.Empty;
    public string Version { get; internal set; } = "1.0.0";
    public string Description { get; internal set; } = string.Empty;
    public string Author { get; internal set; } = "Unknown";
    public string ApiVersion { get; internal set; } = "1.0.0.0";
    public bool IsEnabled { get; internal set; }
    public string DataFolder { get; internal set; } = string.Empty;

    public ILogger Logger { get; internal set; } = null!;
    public IServer Server { get; internal set; } = null!;

    private Config? _config;

    /// <summary>Auto-loaded <c>config.yml</c> inside the plugin data folder.</summary>
    public Config Config => _config ??= Config.Load(Path.Combine(DataFolder, "config.yml"));

    public virtual void OnLoad() { }
    public virtual void OnEnable() { }
    public virtual void OnDisable() { }

    /// <summary>Persists the plugin configuration.</summary>
    public void SaveConfig() => Config.Save(Path.Combine(DataFolder, "config.yml"));

    /// <summary>Reloads the configuration from disk.</summary>
    public void ReloadConfig() => _config = Config.Load(Path.Combine(DataFolder, "config.yml"));

    /// <summary>Schedules a task on the server tick scheduler.</summary>
    public TaskHandler Schedule(Action action, int delayTicks = 1, int periodTicks = 0)
        => periodTicks > 0
            ? Server.Scheduler.RunTaskTimer(this, action, delayTicks, periodTicks)
            : Server.Scheduler.RunTaskLater(this, action, delayTicks);

    public TaskHandlerHelper Tasks => new(this);

    /// <summary>Writes an embedded resource to the plugin data folder.</summary>
    public void SaveResource(string fileName, bool replace = false)
    {
        Directory.CreateDirectory(DataFolder);
        string target = Path.Combine(DataFolder, fileName);
        if (File.Exists(target) && !replace)
        {
            return;
        }
        using Stream? resource = GetResource(fileName);
        if (resource == null)
        {
            Logger.Warning($"Embedded resource '{fileName}' not found");
            return;
        }
        using FileStream file = File.Create(target);
        resource.CopyTo(file);
    }

    public Stream? GetResource(string fileName)
    {
        var assembly = GetType().Assembly;
        string suffix = fileName.Replace('\\', '.');
        string? name = assembly.GetManifestResourceNames()
            .FirstOrDefault(n => n.EndsWith(suffix, StringComparison.OrdinalIgnoreCase));
        return name == null ? null : assembly.GetManifestResourceStream(name);
    }

    public override string ToString() => $"{Name} v{Version}";
}

public readonly struct TaskHandlerHelper
{
    private readonly PluginBase _plugin;
    public TaskHandlerHelper(PluginBase plugin) => _plugin = plugin;

    public Scheduler.TaskHandler Now(Action action) => _plugin.Server.Scheduler.RunTask(_plugin, action);
    public Scheduler.TaskHandler Later(int ticks, Action action) => _plugin.Server.Scheduler.RunTaskLater(_plugin, action, ticks);
    public Scheduler.TaskHandler Timer(int delay, int period, Action action)
        => _plugin.Server.Scheduler.RunTaskTimer(_plugin, action, delay, period);
    public void CancelAll() => _plugin.Server.Scheduler.CancelTasks(_plugin);
}
