using PocketMine.Command;
using PocketMine.Math;
using PocketMine.Player;
using PocketMine.World;

namespace PocketMine.Event;

public abstract class PlayerEvent : Event
{
    public Player.Player Player { get; }
    protected PlayerEvent(Player.Player player) => Player = player;
}

public sealed class PlayerJoinEvent : PlayerEvent
{
    public string JoinMessage { get; set; }
    public override string EventName => "player.join";

    public PlayerJoinEvent(Player.Player player, string joinMessage) : base(player)
        => JoinMessage = joinMessage;
}

public sealed class PlayerQuitEvent : PlayerEvent
{
    public string QuitMessage { get; set; }
    public override string EventName => "player.quit";

    public PlayerQuitEvent(Player.Player player, string quitMessage) : base(player)
        => QuitMessage = quitMessage;
}

public sealed class PlayerChatEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public string Message { get; set; }
    public string Format { get; set; }
    public override string EventName => "player.chat";

    public PlayerChatEvent(Player.Player player, string message, string format = "<{0}> {1}")
    {
        Player = player;
        Message = message;
        Format = format;
    }
}

public sealed class PlayerCommandPreprocessEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public string Message { get; set; }
    public override string EventName => "player.command_preprocess";

    public PlayerCommandPreprocessEvent(Player.Player player, string message)
    {
        Player = player;
        Message = message;
    }
}

public sealed class PlayerMoveEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public Vector3 From { get; }
    public Vector3 To { get; set; }
    public override string EventName => "player.move";

    public PlayerMoveEvent(Player.Player player, Vector3 from, Vector3 to)
    {
        Player = player;
        From = from;
        To = to;
    }
}

public sealed class PlayerInteractEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public BlockPosition Position { get; }
    public string BlockName { get; }
    public string Action { get; }
    public override string EventName => "player.interact";

    public PlayerInteractEvent(Player.Player player, BlockPosition position, string blockName, string action)
    {
        Player = player;
        Position = position;
        BlockName = blockName;
        Action = action;
    }
}

public sealed class BlockBreakEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public BlockPosition Position { get; }
    public uint BlockId { get; }
    public string BlockName { get; }
    public override string EventName => "block.break";

    public BlockBreakEvent(Player.Player player, BlockPosition position, uint blockId, string blockName)
    {
        Player = player;
        Position = position;
        BlockId = blockId;
        BlockName = blockName;
    }
}

public sealed class BlockPlaceEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public BlockPosition Position { get; }
    public uint BlockId { get; }
    public string BlockName { get; }
    public override string EventName => "block.place";

    public BlockPlaceEvent(Player.Player player, BlockPosition position, uint blockId, string blockName)
    {
        Player = player;
        Position = position;
        BlockId = blockId;
        BlockName = blockName;
    }
}

public sealed class EntityDamageEvent : CancellableEvent
{
    public string EntityName { get; }
    public double Damage { get; set; }
    public string Cause { get; }
    public override string EventName => "entity.damage";

    public EntityDamageEvent(string entityName, double damage, string cause)
    {
        EntityName = entityName;
        Damage = damage;
        Cause = cause;
    }
}

public sealed class PlayerDeathEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public string DeathMessage { get; set; }
    public override string EventName => "player.death";

    public PlayerDeathEvent(Player.Player player, string deathMessage)
    {
        Player = player;
        DeathMessage = deathMessage;
    }
}

public sealed class PlayerRespawnEvent : Event
{
    public Player.Player Player { get; }
    public Vector3 Position { get; set; }
    public override string EventName => "player.respawn";

    public PlayerRespawnEvent(Player.Player player, Vector3 position)
    {
        Player = player;
        Position = position;
    }
}

public sealed class ServerCommandEvent : CancellableEvent
{
    public ICommandSender Sender { get; }
    public string Command { get; set; }
    public override string EventName => "server.command";

    public ServerCommandEvent(ICommandSender sender, string command)
    {
        Sender = sender;
        Command = command;
    }
}

public sealed class ServerStartEvent : Event
{
    public override string EventName => "server.start";
}

public sealed class ServerStopEvent : Event
{
    public override string EventName => "server.stop";
}

public sealed class WorldLoadEvent : Event
{
    public World.World World { get; }
    public override string EventName => "world.load";

    public WorldLoadEvent(World.World world) => World = world;
}

public sealed class WorldUnloadEvent : Event
{
    public World.World World { get; }
    public override string EventName => "world.unload";

    public WorldUnloadEvent(World.World world) => World = world;
}
