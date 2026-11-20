using PocketMine.Server;
using PocketMine.Utils;

namespace PocketMine.Plugin;

public abstract class PluginBase
{
    public string Name { get; internal set; } = string.Empty;
    public string Version { get; internal set; } = "1.0.0";
    public string Description { get; internal set; } = string.Empty;
    public string Author { get; internal set; } = string.Empty;
    public bool IsEnabled { get; internal set; } = false;

    public ILogger Logger { get; internal set; } = null!;
    public IServer Server { get; internal set; } = null!;
    public string DataFolder { get; internal set; } = string.Empty;

    public virtual void OnLoad()
    {
    }

    public virtual void OnEnable()
    {
    }

    public virtual void OnDisable()
    {
    }
}
