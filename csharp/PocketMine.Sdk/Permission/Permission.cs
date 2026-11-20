namespace PocketMine.Permission;

public enum PermissionDefault
{
    True,
    False,
    Op,
    NotOp,
}

public sealed class Permission
{
    public string Name { get; }
    public string Description { get; }
    public PermissionDefault Default { get; }
    public Dictionary<string, bool> Children { get; } = new();

    public Permission(string name, string description = "", PermissionDefault @default = PermissionDefault.Op)
    {
        Name = name;
        Description = description;
        Default = @default;
    }

    public Permission AddChild(string node, bool value = true)
    {
        Children[node] = value;
        return this;
    }

    internal static string DefaultName(PermissionDefault value) => value switch
    {
        PermissionDefault.True => "true",
        PermissionDefault.Op => "op",
        PermissionDefault.NotOp => "notop",
        _ => "false",
    };
}
