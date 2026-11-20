using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Plugin;

namespace ExamplePlugin;

[Plugin("ExamplePlugin", "1.0.0", Author = "Zero", Description = "Demonstration C# Plugin running on PocketMine-Rust")]
public class Main : PluginBase
{
    public override void OnLoad()
    {
        Logger.Info("ExamplePlugin is loading...");
    }

    public override void OnEnable()
    {
        Logger.Info("ExamplePlugin enabled successfully!");

        // Register custom C# command
        Server.RegisterCommand(new CSharpInfoCommand());

        // Register event listener
        Server.RegisterEvents(new EventListener(this), this);
    }

    public override void OnDisable()
    {
        Logger.Info("ExamplePlugin disabled.");
    }
}

public class EventListener : IListener
{
    private readonly Main _plugin;

    public EventListener(Main plugin)
    {
        _plugin = plugin;
    }

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnPlayerJoin(PlayerJoinEvent ev)
    {
        _plugin.Logger.Info($"Player '{ev.Player.Name}' has joined the server (caught by C# plugin!)");
        ev.Player.SendMessage($"\x1b[32mWelcome {ev.Player.Name} to PocketMine-Rust! Powered by C# Plugins.\x1b[0m");
    }

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnPlayerChat(PlayerChatEvent ev)
    {
        _plugin.Logger.Info($"[Chat from {ev.Player.Name}]: {ev.Message}");
        if (ev.Message.StartsWith("!ping"))
        {
            ev.Player.SendMessage("\x1b[33mPong from C# Plugin System!\x1b[0m");
            ev.IsCancelled = true;
        }
    }

    [EventHandler(Priority = EventPriority.High)]
    public void OnBlockBreak(BlockBreakEvent ev)
    {
        _plugin.Logger.Debug($"Block broken at {ev.Position} (BlockId: {ev.BlockId}) by {ev.Player.Name}");
    }
}

public class CSharpInfoCommand : Command
{
    public CSharpInfoCommand()
        : base("csharp", "Shows C# Plugin System information", "/csharp", "pocketmine.command.csharp")
    {
    }

    public override bool Execute(ICommandSender sender, string commandLabel, string[] args)
    {
        sender.SendMessage("\x1b[35m=== PocketMine-Rust C# Plugin Host ===\x1b[0m");
        sender.SendMessage("\x1b[36mServer Core:\x1b[0m PocketMine-Rust (High-Performance Async Rust)");
        sender.SendMessage("\x1b[36mPlugin Runtime:\x1b[0m Microsoft .NET 10 CoreCLR (In-Process FFI)");
        sender.SendMessage("\x1b[36mPlugin Status:\x1b[0m ExamplePlugin is running seamlessly!");
        return true;
    }
}
