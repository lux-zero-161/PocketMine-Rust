using PocketMine.Command;
using PocketMine.Inventory;
using PocketMine.Math;
using PocketMine.Server;

namespace PocketMine.Player;

public enum GameMode
{
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 6,
}

public sealed class Player : ICommandSender
{
    public string Name { get; internal set; } = string.Empty;
    public string DisplayName { get; internal set; } = string.Empty;
    public string Uuid { get; internal set; } = string.Empty;
    public string Xuid { get; internal set; } = string.Empty;
    public string Address { get; internal set; } = string.Empty;
    public int Port { get; internal set; }
    public ulong EntityId { get; internal set; }
    public string WorldName { get; internal set; } = string.Empty;

    public Vector3 Position { get; set; } = Vector3.Zero;
    public float Yaw { get; set; }
    public float Pitch { get; set; }
    public float Health { get; internal set; } = 20f;
    public float MaxHealth { get; internal set; } = 20f;
    public int Food { get; internal set; } = 20;
    public int XpLevel { get; internal set; }
    public bool IsOnline { get; internal set; } = true;
    public GameMode GameMode { get; internal set; } = GameMode.Survival;
    public PlayerInventory Inventory { get; internal set; } = new();
    public IServer Server { get; internal set; } = null!;

    public bool IsPlayer => true;
    public bool IsOp { get; internal set; }

    public BlockPosition BlockPosition => new((int)System.Math.Floor(Position.X), (int)System.Math.Floor(Position.Y), (int)System.Math.Floor(Position.Z));

    public void SendMessage(string message) => NativeBridge.SendMessage(Name, message);

    public void SendTip(string message) => NativeBridge.SendMessage(Name, message);

    public void Kick(string reason = "Kicked by server") => NativeBridge.Kick(Name, reason);

    public void Teleport(Vector3 destination)
    {
        Position = destination;
        NativeBridge.Teleport(Name, destination.X, destination.Y, destination.Z);
    }

    public void Teleport(double x, double y, double z) => Teleport(new Vector3(x, y, z));

    public bool HasPermission(string permission) => IsOp || NativeBridge.HasPermission(Name, permission);

    public override string ToString() => $"Player({Name})";
}
