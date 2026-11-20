using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Math;
using PocketMine.Player;
using PocketMine.Plugin;
using PocketMine.Server;
using PocketMine.Utils;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Runtime.Loader;

namespace PocketMine.Host;

public class PluginLogger : ILogger
{
    private readonly string _name;

    public PluginLogger(string name)
    {
        _name = name;
    }

    public void Info(string message) => Entrypoint.Log(1, $"[{_name}] {message}");
    public void Warning(string message) => Entrypoint.Log(3, $"[{_name}] {message}");
    public void Error(string message) => Entrypoint.Log(4, $"[{_name}] {message}");
    public void Debug(string message) => Entrypoint.Log(0, $"[{_name}] {message}");
}

public class RegisteredListener
{
    public PluginBase Plugin { get; }
    public IListener Listener { get; }
    public MethodInfo Method { get; }
    public Type EventType { get; }
    public EventPriority Priority { get; }
    public bool IgnoreCancelled { get; }

    public RegisteredListener(PluginBase plugin, IListener listener, MethodInfo method, Type eventType, EventPriority priority, bool ignoreCancelled)
    {
        Plugin = plugin;
        Listener = listener;
        Method = method;
        EventType = eventType;
        Priority = priority;
        IgnoreCancelled = ignoreCancelled;
    }
}

public class PluginHostServer : IServer
{
    public readonly Dictionary<string, Command.Command> Commands = new(StringComparer.OrdinalIgnoreCase);
    public readonly List<RegisteredListener> Listeners = new();

    public void BroadcastMessage(string message)
    {
        Entrypoint.BroadcastMessage(message);
    }

    public void RegisterCommand(Command.Command command)
    {
        Commands[command.Name] = command;
        Entrypoint.RegisterCommand(command.Name, command.Description, command.UsageMessage, command.Permission);
    }

    public void RegisterEvents(IListener listener, PluginBase plugin)
    {
        var methods = listener.GetType().GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance);
        foreach (var method in methods)
        {
            var attr = method.GetCustomAttribute<EventHandlerAttribute>();
            if (attr == null) continue;

            var parameters = method.GetParameters();
            if (parameters.Length != 1 || !typeof(Event.Event).IsAssignableFrom(parameters[0].ParameterType))
            {
                continue;
            }

            var eventType = parameters[0].ParameterType;
            Listeners.Add(new RegisteredListener(plugin, listener, method, eventType, attr.Priority, attr.IgnoreCancelled));
        }

        Listeners.Sort((a, b) => a.Priority.CompareTo(b.Priority));
    }

    public void Shutdown()
    {
        Entrypoint.Log(1, "Plugin host shutdown requested");
    }
}

public class PluginManager
{
    public readonly List<PluginBase> Plugins = new();
    public readonly PluginHostServer Server = new();

    public void LoadPlugins(string pluginsDirectory)
    {
        if (!Directory.Exists(pluginsDirectory))
        {
            Directory.CreateDirectory(pluginsDirectory);
            return;
        }

        var hostLocation = typeof(PluginManager).Assembly.Location;
        var hostDir = Path.GetDirectoryName(hostLocation) ?? AppContext.BaseDirectory;
        Entrypoint.Log(1, $"HostDir: {hostDir}, scanning directory: {Path.GetFullPath(pluginsDirectory)}");

        var dllFiles = Directory.GetFiles(pluginsDirectory, "*.dll");
        Entrypoint.Log(1, $"Found {dllFiles.Length} dll file(s) in {pluginsDirectory}");
        foreach (var file in dllFiles)
        {
            var fileName = Path.GetFileName(file);
            Entrypoint.Log(1, $"Found plugin candidate: {fileName}");
            if (fileName.Equals("PocketMine.Sdk.dll", StringComparison.OrdinalIgnoreCase) ||
                fileName.Equals("PocketMine.Host.dll", StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            try
            {
                var loadContext = new AssemblyLoadContext(fileName, isCollectible: true);
                loadContext.Resolving += (context, asmName) =>
                {
                    if (asmName.Name == "PocketMine.Sdk")
                    {
                        return typeof(PluginBase).Assembly;
                    }
                    // Check Host directory
                    var inHost = Path.Combine(hostDir, $"{asmName.Name}.dll");
                    if (File.Exists(inHost))
                    {
                        return typeof(PluginManager).Assembly;
                    }
                    // Check plugins directory
                    var inPlugins = Path.Combine(pluginsDirectory, $"{asmName.Name}.dll");
                    if (File.Exists(inPlugins))
                    {
                        return context.LoadFromAssemblyPath(inPlugins);
                    }
                    return null;
                };

                var assembly = loadContext.LoadFromAssemblyPath(Path.GetFullPath(file));

                foreach (var type in assembly.GetTypes())
                {
                    if (type.IsAbstract || !typeof(PluginBase).IsAssignableFrom(type))
                        continue;

                    if (Activator.CreateInstance(type) is PluginBase plugin)
                    {
                        var pluginAttr = type.GetCustomAttribute<PluginAttribute>();
                        plugin.Name = pluginAttr?.Name ?? type.Name;
                        plugin.Version = pluginAttr?.Version ?? "1.0.0";
                        plugin.Author = pluginAttr?.Author ?? "Unknown";
                        plugin.Description = pluginAttr?.Description ?? "";
                        plugin.Logger = new PluginLogger(plugin.Name);
                        plugin.Server = Server;
                        plugin.DataFolder = Path.Combine(pluginsDirectory, plugin.Name);

                        if (!Directory.Exists(plugin.DataFolder))
                        {
                            Directory.CreateDirectory(plugin.DataFolder);
                        }

                        plugin.OnLoad();
                        Plugins.Add(plugin);
                        Entrypoint.Log(1, $"Loaded C# plugin {plugin.Name} v{plugin.Version} by {plugin.Author}");
                    }
                }
            }
            catch (Exception ex)
            {
                Entrypoint.Log(4, $"Failed to load plugin from {fileName}: {ex.Message}");
            }
        }
    }

    public void EnablePlugins()
    {
        foreach (var plugin in Plugins)
        {
            if (plugin.IsEnabled) continue;
            try
            {
                plugin.IsEnabled = true;
                plugin.OnEnable();
                Entrypoint.Log(1, $"Enabled C# plugin {plugin.Name} v{plugin.Version}");
            }
            catch (Exception ex)
            {
                plugin.IsEnabled = false;
                Entrypoint.Log(4, $"Error enabling plugin {plugin.Name}: {ex}");
            }
        }
    }

    public void DisablePlugins()
    {
        for (int i = Plugins.Count - 1; i >= 0; i--)
        {
            var plugin = Plugins[i];
            if (!plugin.IsEnabled) continue;
            try
            {
                plugin.OnDisable();
                plugin.IsEnabled = false;
                Entrypoint.Log(1, $"Disabled C# plugin {plugin.Name}");
            }
            catch (Exception ex)
            {
                Entrypoint.Log(4, $"Error disabling plugin {plugin.Name}: {ex}");
            }
        }
        Plugins.Clear();
        Server.Commands.Clear();
        Server.Listeners.Clear();
    }

    public void DispatchEvent(Event.Event ev)
    {
        foreach (var listener in Server.Listeners)
        {
            if (!listener.Plugin.IsEnabled) continue;
            if (!listener.EventType.IsInstanceOfType(ev)) continue;

            if (ev.IsCancelled && listener.IgnoreCancelled)
                continue;

            try
            {
                listener.Method.Invoke(listener.Listener, new object[] { ev });
            }
            catch (Exception ex)
            {
                Entrypoint.Log(4, $"Error in event handler {listener.Plugin.Name}: {ex.InnerException?.Message ?? ex.Message}");
            }
        }
    }
}
