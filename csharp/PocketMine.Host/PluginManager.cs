using System.Reflection;
using System.Runtime.Loader;
using System.Text.Json;
using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Plugin;
using PocketMine.Server;
using PocketMine.Utils;

namespace PocketMine.Host;

public sealed class PluginManifest
{
    public string? Name { get; set; }
    public string? Version { get; set; }
    public string? Author { get; set; }
    public string? Description { get; set; }
    public string? Api { get; set; }
    public string? Main { get; set; }
    public List<string> Depend { get; set; } = new();

    public static PluginManifest? Load(string directory)
    {
        string path = Path.Combine(directory, "plugin.json");
        if (!File.Exists(path))
        {
            return null;
        }
        try
        {
            return JsonSerializer.Deserialize<PluginManifest>(File.ReadAllText(path), new JsonSerializerOptions
            {
                PropertyNameCaseInsensitive = true,
            });
        }
        catch (Exception ex)
        {
            Native.Log(3, $"Invalid plugin.json in {directory}: {ex.Message}");
            return null;
        }
    }
}

/// <summary>
/// Discovers and loads C# plugin assemblies. Supports both
/// <c>plugins/MyPlugin.dll</c> and <c>plugins/MyPlugin/MyPlugin.dll</c>, with an
/// optional <c>plugin.json</c> manifest next to the assembly.
/// </summary>
public sealed class PluginManager : IPluginManager, IPluginManagerInternal
{
    private sealed class Candidate
    {
        public required Type Type { get; init; }
        public required PluginAttribute? Attribute { get; init; }
        public required PluginManifest? Manifest { get; init; }
        public required string Directory { get; init; }
        public required string AssemblyPath { get; init; }
        public required AssemblyLoadContext Context { get; init; }
    }

    private readonly PluginHostServer _server;
    private readonly List<PluginBase> _plugins = new();
    private readonly List<Candidate> _candidates = new();
    private readonly List<AssemblyLoadContext> _contexts = new();
    private readonly Dictionary<Command.Command, PluginBase> _commandOwners = new();

    public PluginManager(PluginHostServer server)
    {
        _server = server;
    }

    public IReadOnlyList<PluginBase> Plugins => _plugins;

    /// <summary>Plugin currently executing a lifecycle hook, event or command.</summary>
    public static PluginBase? CurrentPlugin { get; internal set; }

    public static void RunAs(PluginBase? plugin, Action action)
    {
        PluginBase? previous = CurrentPlugin;
        CurrentPlugin = plugin;
        try
        {
            action();
        }
        finally
        {
            CurrentPlugin = previous;
        }
    }

    public PluginBase? GetPlugin(string name)
        => _plugins.FirstOrDefault(p => p.Name.Equals(name, StringComparison.OrdinalIgnoreCase));

    public bool IsPluginEnabled(string name) => GetPlugin(name)?.IsEnabled ?? false;

    public PluginBase? GetPluginOf(Command.Command command)
        => _commandOwners.TryGetValue(command, out PluginBase? plugin) ? plugin : null;

    public void TrackCommand(Command.Command command, PluginBase plugin) => _commandOwners[command] = plugin;

    public int LoadPlugins(string directory)
    {
        Directory.CreateDirectory(directory);
        string root = Path.GetFullPath(directory);
        Native.Log(1, $"Scanning C# plugins in {root}");

        var directories = new List<string> { root };
        foreach (string sub in Directory.GetDirectories(root))
        {
            string name = Path.GetFileName(sub);
            if (name is "bin" or "obj" or ".git")
            {
                continue;
            }
            directories.Add(sub);
        }

        foreach (string dir in directories)
        {
            var manifest = dir == root ? null : PluginManifest.Load(dir);
            var context = new AssemblyLoadContext($"PluginContext_{Path.GetFileName(dir)}_{Guid.NewGuid():N}", isCollectible: true);
            context.Resolving += (_, assemblyName) => ResolveAssembly(context, dir, assemblyName);
            _contexts.Add(context);

            foreach (string file in Directory.GetFiles(dir, "*.dll"))
            {
                string fileName = Path.GetFileName(file);
                if (fileName.Equals("PocketMine.Sdk.dll", StringComparison.OrdinalIgnoreCase) ||
                    fileName.Equals("PocketMine.Host.dll", StringComparison.OrdinalIgnoreCase) ||
                    fileName.StartsWith("Microsoft.", StringComparison.OrdinalIgnoreCase) ||
                    fileName.StartsWith("System.", StringComparison.OrdinalIgnoreCase))
                {
                    continue;
                }

                Assembly assembly;
                try
                {
                    assembly = context.LoadFromAssemblyPath(Path.GetFullPath(file));
                }
                catch (BadImageFormatException)
                {
                    continue; // native library sitting next to the plugin
                }
                catch (Exception ex)
                {
                    Native.Log(4, $"Could not load {fileName}: {ex.Message}");
                    continue;
                }

                foreach (Type type in SafeGetTypes(assembly))
                {
                    if (type.IsAbstract || !typeof(PluginBase).IsAssignableFrom(type))
                    {
                        continue;
                    }
                    var attribute = type.GetCustomAttribute<PluginAttribute>();
                    if (manifest?.Main != null && type.FullName != manifest.Main)
                    {
                        continue;
                    }
                    _candidates.Add(new Candidate
                    {
                        Type = type,
                        Attribute = attribute,
                        Manifest = manifest,
                        Directory = dir,
                        AssemblyPath = file,
                        Context = context,
                    });
                }
            }
        }

        if (_candidates.Count == 0)
        {
            Native.Log(1, "No C# plugins found.");
            return 0;
        }

        foreach (Candidate candidate in OrderByDependencies(_candidates))
        {
            Instantiate(candidate);
        }

        return _plugins.Count;
    }

    private static Assembly? ResolveAssembly(AssemblyLoadContext context, string directory, AssemblyName name)
    {
        if (name.Name == "PocketMine.Sdk")
        {
            return typeof(PluginBase).Assembly;
        }
        if (name.Name == "PocketMine.Host")
        {
            return typeof(PluginManager).Assembly;
        }
        string local = Path.Combine(directory, $"{name.Name}.dll");
        if (File.Exists(local))
        {
            return context.LoadFromAssemblyPath(Path.GetFullPath(local));
        }
        string hostDir = Path.GetDirectoryName(typeof(PluginManager).Assembly.Location) ?? AppContext.BaseDirectory;
        string shared = Path.Combine(hostDir, $"{name.Name}.dll");
        if (File.Exists(shared))
        {
            return context.LoadFromAssemblyPath(Path.GetFullPath(shared));
        }
        return null;
    }

    private static IEnumerable<Type> SafeGetTypes(Assembly assembly)
    {
        try
        {
            return assembly.GetTypes();
        }
        catch (ReflectionTypeLoadException ex)
        {
            return ex.Types.Where(t => t != null)!;
        }
    }

    private static List<Candidate> OrderByDependencies(List<Candidate> candidates)
    {
        var byName = new Dictionary<string, Candidate>(StringComparer.OrdinalIgnoreCase);
        foreach (Candidate candidate in candidates)
        {
            string name = PluginName(candidate);
            if (!byName.ContainsKey(name))
            {
                byName[name] = candidate;
            }
        }

        var ordered = new List<Candidate>();
        var visiting = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var visited = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        void Visit(Candidate candidate)
        {
            string name = PluginName(candidate);
            if (visited.Contains(name) || !visiting.Add(name))
            {
                return;
            }
            foreach (string dependency in Dependencies(candidate))
            {
                if (byName.TryGetValue(dependency, out Candidate? dep))
                {
                    Visit(dep);
                }
                else
                {
                    Native.Log(3, $"Plugin {name} depends on {dependency}, which is not installed");
                }
            }
            visiting.Remove(name);
            visited.Add(name);
            ordered.Add(candidate);
        }

        foreach (Candidate candidate in candidates)
        {
            Visit(candidate);
        }
        return ordered;
    }

    private static string PluginName(Candidate candidate)
        => candidate.Attribute?.Name ?? candidate.Manifest?.Name ?? candidate.Type.Name;

    private static IEnumerable<string> Dependencies(Candidate candidate)
    {
        if (candidate.Attribute?.Depends is { Length: > 0 } depends)
        {
            return depends;
        }
        return candidate.Manifest?.Depend ?? (IEnumerable<string>)Array.Empty<string>();
    }

    private void Instantiate(Candidate candidate)
    {
        string name = PluginName(candidate);
        if (_plugins.Any(p => p.Name.Equals(name, StringComparison.OrdinalIgnoreCase)))
        {
            Native.Log(3, $"Duplicate plugin name '{name}' ignored ({candidate.AssemblyPath})");
            return;
        }

        string api = candidate.Attribute?.Api ?? candidate.Manifest?.Api ?? "1.0.0.0";
        if (!ApiCompatible(api))
        {
            Native.Log(4, $"Plugin {name} requires API {api}, server provides {pmVersions.ApiVersion}");
            return;
        }

        try
        {
            if (Activator.CreateInstance(candidate.Type) is not PluginBase plugin)
            {
                Native.Log(4, $"Could not instantiate plugin type {candidate.Type.FullName}");
                return;
            }

            plugin.Name = name;
            plugin.Version = candidate.Attribute?.Version ?? candidate.Manifest?.Version ?? "1.0.0";
            plugin.Author = candidate.Attribute?.Author ?? candidate.Manifest?.Author ?? "Unknown";
            plugin.Description = candidate.Attribute?.Description ?? candidate.Manifest?.Description ?? "";
            plugin.ApiVersion = api;
            plugin.Logger = new PluginLogger(name);
            plugin.Server = _server;
            plugin.DataFolder = Path.Combine(candidate.Directory, name);
            Directory.CreateDirectory(plugin.DataFolder);

            RunAs(plugin, plugin.OnLoad);
            _plugins.Add(plugin);
            Native.Log(1, $"Loaded C# plugin {plugin.Name} v{plugin.Version} by {plugin.Author} [API {api}]");
        }
        catch (Exception ex)
        {
            Native.Log(4, $"Error loading plugin {name}: {ex}");
        }
    }

    private static bool ApiCompatible(string required)
    {
        static int Major(string version) => int.TryParse(version.Split('.')[0], out int major) ? major : -1;
        return Major(required) == Major(pmVersions.ApiVersion);
    }

    public void EnablePlugins()
    {
        foreach (PluginBase plugin in _plugins)
        {
            if (plugin.IsEnabled)
            {
                continue;
            }
            try
            {
                plugin.IsEnabled = true;
                RunAs(plugin, plugin.OnEnable);
                Native.Log(1, $"Enabled C# plugin {plugin.Name} v{plugin.Version}");
            }
            catch (Exception ex)
            {
                plugin.IsEnabled = false;
                Native.Log(4, $"Error enabling plugin {plugin.Name}: {ex}");
            }
        }
    }

    public void DisablePlugins()
    {
        for (int i = _plugins.Count - 1; i >= 0; i--)
        {
            PluginBase plugin = _plugins[i];
            try
            {
                if (plugin.IsEnabled)
                {
                    RunAs(plugin, plugin.OnDisable);
                }
            }
            catch (Exception ex)
            {
                Native.Log(4, $"Error disabling plugin {plugin.Name}: {ex}");
            }
            plugin.IsEnabled = false;
            if (_server.Scheduler is HostTaskScheduler scheduler)
            {
                scheduler.CancelTasks(plugin);
            }
            UnregisterPlugin(plugin);
        }
        _plugins.Clear();
        _candidates.Clear();
        _commandOwners.Clear();
        // Collectible load contexts allow /reload without leaking the plugin
        // assemblies; all listeners/commands referencing them are gone by now.
        foreach (AssemblyLoadContext context in _contexts)
        {
            try
            {
                context.Unload();
            }
            catch (Exception ex)
            {
                Native.Log(3, $"Could not unload plugin context: {ex.Message}");
            }
        }
        _contexts.Clear();
    }

    public void UnregisterPlugin(PluginBase plugin)
    {
        foreach (var command in _commandOwners.Where(kv => ReferenceEquals(kv.Value, plugin)).Select(kv => kv.Key).ToList())
        {
            _server.Commands.Remove(command.Name);
            Native.UnregisterCommand(command.Name);
            _commandOwners.Remove(command);
        }
        _server.Listeners.RemoveAll(listener => ReferenceEquals(listener.Plugin, plugin));
    }

    public void DispatchEvent(Event.Event ev)
    {
        // Copy because handlers may register or remove listeners.
        foreach (RegisteredListener listener in _server.Listeners.ToArray())
        {
            if (!listener.Plugin.IsEnabled || !listener.EventType.IsInstanceOfType(ev))
            {
                continue;
            }
            if (ev.IsCancelled && listener.IgnoreCancelled)
            {
                continue;
            }
            try
            {
                RunAs(listener.Plugin, () => listener.Method.Invoke(listener.Listener, new object[] { ev }));
            }
            catch (TargetInvocationException ex)
            {
                Native.Log(4, $"Error in event handler {listener.Plugin.Name}::{listener.Method.Name}: {ex.InnerException?.Message ?? ex.Message}");
            }
            catch (Exception ex)
            {
                Native.Log(4, $"Error dispatching {ev.EventName} to {listener.Plugin.Name}: {ex.Message}");
            }
        }
    }

    public string ListPluginsJson()
    {
        var list = _plugins.Select(plugin => new
        {
            name = plugin.Name,
            version = plugin.Version,
            author = plugin.Author,
            description = plugin.Description,
            api = plugin.ApiVersion,
            enabled = plugin.IsEnabled,
            commands = _server.Commands.Values
                .Where(command => ReferenceEquals(GetPluginOf(command), plugin))
                .Select(command => command.Name)
                .ToArray(),
        });
        return JsonSerializer.Serialize(list);
    }
}

public interface IPluginManagerInternal
{
    void TrackCommand(Command.Command command, PluginBase plugin);
}

public sealed class PluginLogger : ILogger
{
    private readonly string _name;

    public PluginLogger(string name) => _name = name;

    public void Info(string message) => Native.Log(1, $"[{_name}] {message}");
    public void Warning(string message) => Native.Log(3, $"[{_name}] {message}");
    public void Error(string message) => Native.Log(4, $"[{_name}] {message}");
    public void Debug(string message) => Native.Log(0, $"[{_name}] {message}");
}
