using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Permission;
using PocketMine.Plugin;
using PocketMine.Scheduler;
using PocketMine.Utils;
using PocketMine.Player;
using PocketMine.World;

namespace PocketMine.Server;

public interface IPluginManager
{
    IReadOnlyList<PluginBase> Plugins { get; }
    PluginBase? GetPlugin(string name);
    bool IsPluginEnabled(string name);
}

public interface IServer
{
    ServerInfo Info { get; }
    string Name { get; }
    string Version { get; }
    string ApiVersion { get; }
    string MinecraftVersion { get; }
    int Protocol { get; }
    ulong Tick { get; }
    double Tps { get; }
    int MaxPlayers { get; }
    int PlayerCount { get; }
    IReadOnlyList<Player.Player> OnlinePlayers { get; }
    IReadOnlyList<World.World> Worlds { get; }
    World.World? DefaultWorld { get; }
    World.World? GetWorld(string name);
    Player.Player? GetPlayer(string name);
    Player.Player? GetPlayerExact(string name);
    Player.Player? GetPlayerByUuid(string uuid);
    IReadOnlyList<string> PlayerNames { get; }
    ILogger Logger { get; }
    ITaskScheduler Scheduler { get; }
    IPluginManager PluginManager { get; }
    void BroadcastMessage(string message);
    bool DispatchCommand(ICommandSender sender, string commandLine);
    void RegisterCommand(Command.Command command);
    void UnregisterCommand(string name);
    void RegisterEvents(IListener listener, PluginBase plugin);
    void RegisterPermission(Permission.Permission permission);
    long GetWorldTime(string world);
    void SetWorldTime(string world, long time);
    void Shutdown();
}

/// <summary>Delegates filled in by the .NET host; used by SDK model objects.</summary>
public static class NativeBridge
{
    internal static Func<string, string, bool>? SendMessageAction;
    internal static Func<string, string, bool>? KickAction;
    internal static Func<string, double, double, double, bool>? TeleportAction;
    internal static Func<string, string, bool>? HasPermissionAction;

    public static bool SendMessage(string player, string message) => SendMessageAction?.Invoke(player, message) ?? false;
    public static bool Kick(string player, string reason) => KickAction?.Invoke(player, reason) ?? false;
    public static bool Teleport(string player, double x, double y, double z) => TeleportAction?.Invoke(player, x, y, z) ?? false;
    public static bool HasPermission(string player, string permission) => HasPermissionAction?.Invoke(player, permission) ?? false;
}
