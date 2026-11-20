namespace PocketMine.Plugin;

[AttributeUsage(AttributeTargets.Class, Inherited = false, AllowMultiple = false)]
public sealed class PluginAttribute : Attribute
{
    public string Name { get; }
    public string Version { get; }
    public string Author { get; set; } = "Unknown";
    public string Description { get; set; } = "";

    public PluginAttribute(string name, string version)
    {
        Name = name;
        Version = version;
    }
}
