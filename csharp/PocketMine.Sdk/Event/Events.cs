using PocketMine.Command;
using PocketMine.Math;
using PocketMine.Player;

namespace PocketMine.Event;

public class PlayerJoinEvent : Event
{
    public Player.Player Player { get; }
    public string JoinMessage { get; set; }

    public PlayerJoinEvent(Player.Player player, string joinMessage)
    {
        Player = player;
        JoinMessage = joinMessage;
    }
}

public class PlayerQuitEvent : Event
{
    public Player.Player Player { get; }
    public string QuitMessage { get; set; }

    public PlayerQuitEvent(Player.Player player, string quitMessage)
    {
        Player = player;
        QuitMessage = quitMessage;
    }
}

public class PlayerChatEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public string Message { get; set; }
    public string Format { get; set; }

    public PlayerChatEvent(Player.Player player, string message, string format = "<{0}> {1}")
    {
        Player = player;
        Message = message;
        Format = format;
    }
}

public class BlockBreakEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public Vector3 Position { get; }
    public uint BlockId { get; }

    public BlockBreakEvent(Player.Player player, Vector3 position, uint blockId)
    {
        Player = player;
        Position = position;
        BlockId = blockId;
    }
}

public class BlockPlaceEvent : CancellableEvent
{
    public Player.Player Player { get; }
    public Vector3 Position { get; }
    public uint BlockId { get; }

    public BlockPlaceEvent(Player.Player player, Vector3 position, uint blockId)
    {
        Player = player;
        Position = position;
        BlockId = blockId;
    }
}

public class ServerCommandEvent : CancellableEvent
{
    public ICommandSender Sender { get; }
    public string Command { get; set; }

    public ServerCommandEvent(ICommandSender sender, string command)
    {
        Sender = sender;
        Command = command;
    }
}
