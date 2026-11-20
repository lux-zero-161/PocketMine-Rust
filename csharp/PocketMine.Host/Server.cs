using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Player;
using PocketMine.Plugin;
using PocketMine.Scheduler;
using PocketMine.Server;
using PocketMine.Utils;
using PocketMine.World;

namespace PocketMine.Host;

public sealed class ServerLogger : ILogger
{
    public void Info(string message) => Entrypoint.Log(1, message);
    public void Warning(string message) => Entrypoint.Log(3, message);
    public void Error(string message) => Entrypoint.Log(4, message);
    public void Debug(string message) => Entrypoint.Log(0, message);
}

public sealed class RegisteredListener
{
    public PluginBase Plugin { get; }
    public IListener Listener { get; }
    public System.Reflection.MethodInfo Method { get; }
    public Type EventType { get; }
    public EventPriority Priority { get; }
    public bool IgnoreCancelled { get; }

    public RegisteredListener(PluginBase plugin, IListener listener, System.Reflection.MethodInfo method, Type eventType, EventPriority priority, bool ignoreCancelled)
    {
        Plugin = plugin;
        Listener = listener;
        Method = method;
        EventType = eventType;
        Priority = priority;
        IgnoreCancelled = ignoreCancelled;
    }
}

/// <summary>Implementation of <see cref="IServer"/> exposed to plugins.</summary>
public sealed class PluginHostServer : IServer
{
    private readonly HostTaskScheduler _scheduler = new();
    public HostPermissionRegistry Permissions { get; } = new();
    private readonly ServerLogger _logger = new();
    private readonly ConsoleCommandSender _console;

    public PluginManager Manager { get; internal set; } = null!;
    public readonly Dictionary<string, Command.Command> Commands = new(StringComparer.OrdinalIgnoreCase);
    public readonly List<RegisteredListener> Listeners = new();

    public PluginHostServer()
    {
        _console = new ConsoleCommandSender(message => Native.Log(1, message), () => this);
    }

    public ICommandSender ConsoleSender => _console;

    public string Name => "PocketMine-Rust";
    public string Version => pmVersions.Version;
    public string ApiVersion => pmVersions.ApiVersion;
    public string MinecraftVersion => pmVersions.MinecraftVersion;
    public int Protocol => pmVersions.Protocol;
    public ulong Tick => (ulong)Info.Tick;
    public double Tps => Info.Tps;
    public int MaxPlayers => Info.MaxPlayers;
    public int PlayerCount => OnlinePlayers.Count;
    public long Uptime => Info.Uptime;
    public string Motd => Info.Motd;

    private ServerInfo? _cachedInfo;
    private DateTime _infoFetchedAt = DateTime.MinValue;

    public ServerInfo Info
    {
        get
        {
            if (_cachedInfo == null || (DateTime.UtcNow - _infoFetchedAt).TotalMilliseconds > 250)
            {
                _cachedInfo = CacheServerInfo();
                _infoFetchedAt = DateTime.UtcNow;
            }
            return _cachedInfo;
        }
    }

    private void InvalidateInfo() => _infoFetchedAt = DateTime.MinValue;

    private ServerInfo CacheServerInfo()
    {
        ServerInfo info = JsonModels.ServerInfo(Native.GetServerInfo());
        // Populate plugin list from the managed side (authoritative during startup).
        return info with
        {
            Plugins = Manager == null ? info.Plugins : Manager.Plugins.Select(p => p.Name).ToArray(),
        };
    }

    public IReadOnlyList<Player.Player> OnlinePlayers
    {
        get
        {
            string json = Native.GetPlayers();
            using var document = System.Text.Json.JsonDocument.Parse(json);
            return document.RootElement.EnumerateArray()
                .Select(element => JsonModels.Player(element, this))
                .ToArray();
        }
    }

    public IReadOnlyList<string> PlayerNames => OnlinePlayers.Select(p => p.Name).ToArray();

    public IReadOnlyList<World.World> Worlds
    {
        get
        {
            string json = Native.GetWorlds();
            using var document = System.Text.Json.JsonDocument.Parse(json);
            return document.RootElement.EnumerateArray()
                .Select(JsonModels.World)
                .ToArray();
        }
    }

    public World.World? DefaultWorld => Worlds.FirstOrDefault();

    public ILogger Logger => _logger;
    public ITaskScheduler Scheduler => _scheduler;

    public IPluginManager PluginManager => Manager;

    public World.World? GetWorld(string name) => Worlds.FirstOrDefault(w => w.Name.Equals(name, StringComparison.OrdinalIgnoreCase));

    public Player.Player? GetPlayer(string name)
    {
        string? json = Native.GetPlayer(name);
        if (json == null) return null;
        using var document = System.Text.Json.JsonDocument.Parse(json);
        return JsonModels.Player(document.RootElement, this);
    }

    public Player.Player? GetPlayerExact(string name)
    {
        return OnlinePlayers.FirstOrDefault(p => p.Name == name);
    }

    public Player.Player? GetPlayerByUuid(string uuid) => OnlinePlayers.FirstOrDefault(p => p.Uuid == uuid);

    public void BroadcastMessage(string message) => Native.Broadcast(message);

    public bool DispatchCommand(ICommandSender sender, string commandLine)
        => Native.DispatchCommand(sender.Name, commandLine, sender.IsPlayer);

    public void RegisterCommand(Command.Command command)
    {
        Commands[command.Name] = command;
        string aliases = string.Join(",", command.Aliases);
        PluginBase? owner = PocketMine.Host.PluginManager.CurrentPlugin ?? Manager.GetPluginOf(command);
        if (owner != null)
        {
            Manager.TrackCommand(command, owner);
        }
        string plugin = owner?.Name ?? "unknown";
        if (!Native.RegisterCommand(command.Name, command.Description, command.UsageMessage, command.Permission ?? "", aliases, plugin))
        {
            Native.Log(3, $"Failed to register command /{command.Name}");
        }
    }

    public void UnregisterCommand(string name)
    {
        Commands.Remove(name);
        Native.UnregisterCommand(name);
    }

    public void RegisterEvents(IListener listener, PluginBase plugin)
    {
        foreach (var method in listener.GetType().GetMethods(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance))
        {
            var attribute = method.GetCustomAttributes(typeof(EventHandlerAttribute), false).OfType<EventHandlerAttribute>().FirstOrDefault();
            if (attribute == null)
            {
                continue;
            }
            var parameters = method.GetParameters();
            if (parameters.Length != 1 || !typeof(Event.Event).IsAssignableFrom(parameters[0].ParameterType))
            {
                Native.Log(3, $"Invalid event handler {listener.GetType().Name}.{method.Name}: needs exactly one Event parameter");
                continue;
            }
            Listeners.Add(new RegisteredListener(plugin, listener, method, parameters[0].ParameterType, attribute.Priority, attribute.IgnoreCancelled));
        }
        Listeners.Sort((a, b) => a.Priority.CompareTo(b.Priority));
    }

    public void RegisterPermission(Permission.Permission permission)
    {
        Permissions.Register(permission.Name, permission.Description, permission.Default);
        Native.RegisterPermission(permission.Name, permission.Description, Permission.Permission.DefaultName(permission.Default));
        foreach (var child in permission.Children)
        {
            Permissions.Register(child.Key, permission.Description, child.Value ? PocketMine.Permission.PermissionDefault.True : PocketMine.Permission.PermissionDefault.False);
            Native.RegisterPermission(child.Key, permission.Description, child.Value ? "true" : "false");
        }
    }

    public long GetWorldTime(string world) => GetWorld(world)?.Time ?? 0;

    public void SetWorldTime(string world, long time)
    {
        DispatchCommand(_console, $"world settime {time}");
    }

    public void Shutdown()
    {
        Native.Log(1, "Plugin requested server shutdown");
        Native.Shutdown();
    }
}

/// <summary>Version constants shared with the Rust core.</summary>
public static class pmVersions
{
    public const string Version = "1.0.0.0";
    public const string ApiVersion = "1.0.0.0";
    public const string MinecraftVersion = "1.26.60";
    public const int Protocol = 2225;
}
