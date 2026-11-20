using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Math;
using PocketMine.Permission;
using PocketMine.Plugin;
using PocketMine.Utils;
using PocketMine.World;

namespace ExamplePlugin;

[Plugin(
    "ExamplePlugin",
    "1.0.0",
    Author = "Zero",
    Description = "Reference C# plugin for PocketMine-Rust",
    Api = "1.0.0.0"
)]
public class Main : PluginBase
{
    private int _ticks;
    private int _blocksPlaced;

    public override void OnLoad()
    {
        Logger.Info("Loading configuration and resources...");
        if (!Config.Has("welcome-message"))
        {
            Config.Set("welcome-message", "§aWelcome to PocketMine-Rust, {player}!");
            Config.Set("announce-tps", true);
            SaveConfig();
        }
        Logger.Info($"Data folder: {DataFolder}");
    }

    public override void OnEnable()
    {
        Logger.Info("ExamplePlugin enabled.");

        Server.RegisterPermission(new Permission(
            "exampleplugin.command.csharp",
            "Allows using the /csharp command",
            PermissionDefault.True));
        Server.RegisterPermission(new Permission(
            "exampleplugin.command.build",
            "Allows using the /build command",
            PermissionDefault.Op));

        Server.RegisterCommand(new CSharpInfoCommand());
        Server.RegisterCommand(new HelloCommand(this));
        Server.RegisterCommand(new BuildCommand(this));

        Server.RegisterEvents(new EventListener(this), this);

        // Demonstrates the C# task scheduler: runs every 5 seconds.
        Server.Scheduler.RunTaskTimer(this, () =>
        {
            _ticks++;
            if (Config.GetBool("announce-tps", true) && _ticks % 6 == 0)
            {
                Logger.Debug($"Server tick {Server.Tick}, TPS {Server.Tps:F2}, players {Server.PlayerCount}/{Server.MaxPlayers}");
            }
        }, 20, 100);

        Logger.Info($"Registered commands and listeners. Server: {Server.Info}");
    }

    public override void OnDisable()
    {
        Logger.Info($"ExamplePlugin disabled after {_ticks} scheduled tasks ({_blocksPlaced} blocks placed).");
    }

    public int BlocksPlaced => _blocksPlaced;
    internal void CountBlockPlaced() => _blocksPlaced++;
}

public class EventListener : IListener
{
    private readonly Main _plugin;

    public EventListener(Main plugin) => _plugin = plugin;

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnPlayerJoin(PlayerJoinEvent ev)
    {
        string template = _plugin.Config.GetString("welcome-message", "Welcome, {player}!");
        string message = template.Replace("{player}", ev.Player.Name);
        ev.JoinMessage = $"§e{ev.Player.Name} joined the game §7(via C# event)";
        ev.Player.SendMessage(message);
        ev.Player.SendMessage($"§7Position: {ev.Player.Position}, world: {ev.Player.WorldName}");
        _plugin.Logger.Info($"Player '{ev.Player.Name}' joined (handled in C#)");
    }

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnPlayerQuit(PlayerQuitEvent ev)
    {
        ev.QuitMessage = $"§e{ev.Player.Name} left the game";
        _plugin.Logger.Info($"Player '{ev.Player.Name}' quit");
    }

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnPlayerChat(PlayerChatEvent ev)
    {
        string message = ev.Message;
        if (message.Equals("!ping", StringComparison.OrdinalIgnoreCase))
        {
            ev.Player.SendMessage("§bPong from C# running in the Rust core!");
            ev.IsCancelled = true;
            return;
        }
        if (message.Equals("!pos", StringComparison.OrdinalIgnoreCase))
        {
            Vector3 pos = ev.Player.Position;
            ev.Player.SendMessage($"§7You are at §f{pos.X:F1}, §f{pos.Y:F1}, §f{pos.Z:F1} §7in §f{ev.Player.WorldName}");
            ev.IsCancelled = true;
            return;
        }
        if (message.Equals("!time", StringComparison.OrdinalIgnoreCase))
        {
            World? world = ev.Player.Server.GetWorld(ev.Player.WorldName);
            ev.Player.SendMessage($"§7World time: §f{world?.Time ?? 0}");
            ev.IsCancelled = true;
            return;
        }
        _plugin.Logger.Debug($"[chat] {ev.Player.Name}: {message}");
    }

    [EventHandler(Priority = EventPriority.High, IgnoreCancelled = true)]
    public void OnBlockBreak(BlockBreakEvent ev)
    {
        if (ev.BlockName == "minecraft:bedrock")
        {
            ev.Player.SendMessage("§cBedrock cannot be broken.");
            ev.IsCancelled = true;
            return;
        }
        _plugin.Logger.Debug($"Block {ev.BlockName} broken at {ev.Position} by {ev.Player.Name}");
    }

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnBlockPlace(BlockPlaceEvent ev)
    {
        _plugin.CountBlockPlaced();
    }

    [EventHandler(Priority = EventPriority.Monitor)]
    public void OnCommand(PlayerCommandPreprocessEvent ev)
    {
        _plugin.Logger.Debug($"{ev.Player.Name} ran: {ev.Message}");
    }

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnDamage(EntityDamageEvent ev)
    {
        if (ev.Cause == "fall" && ev.Damage > 10)
        {
            ev.Damage = 10;
        }
    }

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnWorldLoad(WorldLoadEvent ev)
    {
        _plugin.Logger.Info($"World loaded: {ev.World.Name}");
    }
}

public class CSharpInfoCommand : Command
{
    public CSharpInfoCommand()
        : base("csharp", "Shows C# plugin host information", "/csharp", "exampleplugin.command.csharp")
    {
        Aliases.Add("cs");
    }

    public override bool Execute(ICommandSender sender, string commandLabel, string[] args)
    {
        sender.SendMessage("§5=== PocketMine-Rust · C# plugin host ===");
        sender.SendMessage("§bEngine:§r PocketMine-Rust §7(Rust core, in-process CoreCLR)");
        sender.SendMessage("§bPlugin API:§r PocketMine.Sdk 1.0.0.0");
        sender.SendMessage("§bRuntime:§r .NET 10 / UnmanagedCallersOnly ABI");
        sender.SendMessage("§bPlugins:§r " + string.Join(", ", sender.Server.PluginManager.Plugins.Select(x => x.Name)));
        IReadOnlyList<string> worlds = sender.Server.Info.Worlds;
        sender.SendMessage("§bWorlds:§r " + string.Join(", ", worlds));
        return true;
    }
}

public class HelloCommand : Command
{
    private readonly Main _plugin;

    public HelloCommand(Main plugin)
        : base("hello", "Greets a player from C#", "/hello [player]")
    {
        _plugin = plugin;
    }

    public override bool Execute(ICommandSender sender, string commandLabel, string[] args)
    {
        string target = args.Length > 0 ? args[0] : sender.Name;
        sender.SendMessage($"§aHello, §e{target}§a! This line came from C# plugin '{_plugin.Name}'.");
        return true;
    }
}

public class BuildCommand : Command
{
    private readonly Main _plugin;

    public BuildCommand(Main plugin)
        : base("build", "Places a small stone platform with the world API", "/build [size]", "exampleplugin.command.build")
    {
        _plugin = plugin;
    }

    public override bool Execute(ICommandSender sender, string commandLabel, string[] args)
    {
        if (sender is not PocketMine.Player.Player player)
        {
            sender.SendMessage("§cRun this in-game.");
            return false;
        }

        int size = args.Length > 0 && int.TryParse(args[0], out int parsed) ? System.Math.Clamp(parsed, 1, 8) : 3;
        World? world = player.Server.GetWorld(player.WorldName);
        if (world == null)
        {
            sender.SendMessage("§cWorld not found.");
            return false;
        }

        BlockPosition origin = player.BlockPosition;
        for (int x = -size; x <= size; x++)
        {
            for (int z = -size; z <= size; z++)
            {
                world.SetBlock(origin.X + x, origin.Y - 1, origin.Z + z, "minecraft:stone");
            }
        }

        sender.SendMessage($"§aBuilt a §e{size * 2 + 1}x{size * 2 + 1}§a stone platform at §e{origin}");
        _plugin.Logger.Info($"{player.Name} built a platform at {origin}");
        return true;
    }
}
