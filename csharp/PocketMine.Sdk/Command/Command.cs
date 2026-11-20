namespace PocketMine.Command;

public interface ICommandSender
{
    Server.IServer Server { get; }
    string Name { get; }
    void SendMessage(string message);
    bool HasPermission(string permission);
    bool IsPlayer { get; }
    bool IsOp { get; }
}

/// <summary>Sender used by the console and remote-console commands.</summary>
public sealed class ConsoleCommandSender : ICommandSender
{
    private readonly Action<string> _write;
    private readonly Func<Server.IServer> _server;

    public ConsoleCommandSender(Action<string> write, Func<Server.IServer>? server = null)
    {
        _write = write;
        _server = server ?? (() => throw new InvalidOperationException("Console sender is not attached to a server"));
    }

    public Server.IServer Server => _server();
    public string Name => "CONSOLE";
    public bool IsPlayer => false;
    public bool IsOp => true;
    public void SendMessage(string message) => _write(message);
    public bool HasPermission(string permission) => true;
}

public abstract class Command
{
    public string Name { get; }
    public string Description { get; set; }
    public string UsageMessage { get; set; }
    public string? Permission { get; set; }
    public string PermissionMessage { get; set; } = "§cYou don't have permission to use this command.";
    public List<string> Aliases { get; } = new();

    protected Command(string name, string description = "", string usageMessage = "", string? permission = null)
    {
        Name = name;
        Description = description;
        UsageMessage = usageMessage.Length > 0 ? usageMessage : $"/{name}";
        Permission = permission;
    }

    public bool TestPermission(ICommandSender sender)
    {
        if (Permission == null || sender.HasPermission(Permission))
        {
            return true;
        }
        sender.SendMessage(PermissionMessage);
        return false;
    }

    public abstract bool Execute(ICommandSender sender, string commandLabel, string[] args);

    /// <summary>Tab completion hook; return an empty sequence for no suggestions.</summary>
    public virtual IEnumerable<string> OnTabComplete(ICommandSender sender, string alias, string[] args) => Array.Empty<string>();
}
