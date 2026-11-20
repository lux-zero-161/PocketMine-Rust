namespace PocketMine.Command;

public abstract class Command
{
    public string Name { get; }
    public string Description { get; set; }
    public string UsageMessage { get; set; }
    public string? Permission { get; set; }
    public List<string> Aliases { get; } = new();

    protected Command(string name, string description = "", string usageMessage = "", string? permission = null)
    {
        Name = name;
        Description = description;
        UsageMessage = usageMessage.Length > 0 ? usageMessage : $"/{name}";
        Permission = permission;
    }

    public abstract bool Execute(ICommandSender sender, string commandLabel, string[] args);
}
