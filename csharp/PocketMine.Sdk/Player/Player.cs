using PocketMine.Command;
using PocketMine.Inventory;
using PocketMine.Math;

namespace PocketMine.Player;

public class Player : ICommandSender
{
    public string Name { get; }
    public string Uuid { get; }
    public Vector3 Position { get; set; }
    public float Health { get; set; } = 20.0f;
    public int GameMode { get; set; } = 0;
    public PlayerInventory Inventory { get; } = new();
    public bool IsPlayer => true;

    private readonly Action<string, string> _sendMessageAction;
    private readonly Action<string, string> _kickAction;
    private readonly Action<string, double, double, double> _teleportAction;

    public Player(
        string name,
        string uuid,
        Vector3 position,
        Action<string, string> sendMessageAction,
        Action<string, string> kickAction,
        Action<string, double, double, double> teleportAction)
    {
        Name = name;
        Uuid = uuid;
        Position = position;
        _sendMessageAction = sendMessageAction;
        _kickAction = kickAction;
        _teleportAction = teleportAction;
    }

    public void SendMessage(string message) => _sendMessageAction(Name, message);

    public void Kick(string reason = "Kicked by server") => _kickAction(Name, reason);

    public void Teleport(Vector3 destination)
    {
        Position = destination;
        _teleportAction(Name, destination.X, destination.Y, destination.Z);
    }

    public bool HasPermission(string permission) => true;
}
