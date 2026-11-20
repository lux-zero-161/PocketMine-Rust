namespace PocketMine.Command;

public interface ICommandSender
{
    string Name { get; }
    void SendMessage(string message);
    bool HasPermission(string permission);
    bool IsPlayer { get; }
}
