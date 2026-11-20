using System.Runtime.InteropServices;
using System.Text.Json;
using PocketMine.Command;
using PocketMine.Player;
using PocketMine.Server;

namespace PocketMine.Host;

/// <summary>
/// Managed entry points called by the Rust core through <c>hostfxr</c>.
/// Every method marked <c>[UnmanagedCallersOnly]</c> is resolved with
/// <c>UNMANAGEDCALLERSONLY_METHOD</c> by <c>pm_plugin_host</c>.
/// </summary>
public static unsafe class Entrypoint
{
    private static PluginHostServer? _server;
    private static PluginManager? _manager;

    public static void Log(int level, string message)
    {
        if (Native.IsInitialized)
        {
            Native.Log(level, message);
        }
        else
        {
            Console.WriteLine(message);
        }
    }

    [UnmanagedCallersOnly]
    public static int InitializeHost(IntPtr apiTablePtr)
    {
        try
        {
            if (apiTablePtr == IntPtr.Zero)
            {
                return -1;
            }

            var table = *(RustApiTable*)apiTablePtr;
            Native.Initialize(table);

            NativeBridge.SendMessageAction = Native.SendMessage;
            NativeBridge.KickAction = Native.Kick;
            NativeBridge.TeleportAction = Native.Teleport;
            NativeBridge.HasPermissionAction = (player, permission) =>
            {
                Player.Player? p = _server?.GetPlayer(player);
                return _server != null && _server.Permissions.HasPermission(p, permission);
            };

            _server = new PluginHostServer();
            _manager = new PluginManager(_server);
            _server.Manager = _manager;

            Native.Log(1, "C# plugin host initialized (CoreCLR, in-process)");
            Native.Log(1, $"Plugin API {pmVersions.ApiVersion} | Minecraft {pmVersions.MinecraftVersion} | protocol {pmVersions.Protocol}");
            return 0;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"InitializeHost failed: {ex}");
            return -1;
        }
    }

    [UnmanagedCallersOnly]
    public static int LoadPlugins(IntPtr directoryUtf8)
    {
        try
        {
            string directory = Marshal.PtrToStringUTF8(directoryUtf8) ?? "plugins";
            return _manager?.LoadPlugins(directory) ?? 0;
        }
        catch (Exception ex)
        {
            Log(4, $"LoadPlugins failed: {ex}");
            return 0;
        }
    }

    [UnmanagedCallersOnly]
    public static int EnablePlugins()
    {
        try
        {
            _manager?.EnablePlugins();
            return 0;
        }
        catch (Exception ex)
        {
            Log(4, $"EnablePlugins failed: {ex}");
            return -1;
        }
    }

    [UnmanagedCallersOnly]
    public static int DisablePlugins()
    {
        try
        {
            _manager?.DisablePlugins();
            return 0;
        }
        catch (Exception ex)
        {
            Log(4, $"DisablePlugins failed: {ex}");
            return -1;
        }
    }

    [UnmanagedCallersOnly]
    public static int DispatchEvent(int eventType, IntPtr jsonUtf8, IntPtr cancelOut, IntPtr resultBuffer, int resultCapacity)
    {
        try
        {
            if (_manager == null || _server == null)
            {
                return -1;
            }
            string json = Marshal.PtrToStringUTF8(jsonUtf8) ?? "{}";
            using var document = JsonDocument.Parse(json);
            Event.Event? ev = EventFactory.Create(eventType, document.RootElement, _server);
            if (ev == null)
            {
                return -1;
            }
            _manager.DispatchEvent(ev);
            if (cancelOut != IntPtr.Zero)
            {
                *(int*)cancelOut = ev.IsCancellable && ev.IsCancelled ? 1 : 0;
            }

            byte[]? patch = EventPatcher.Patch(eventType, ev);
            if (patch == null || patch.Length == 0 || resultBuffer == IntPtr.Zero || resultCapacity <= 0)
            {
                return 0;
            }
            int length = System.Math.Min(patch.Length, resultCapacity);
            Marshal.Copy(patch, 0, resultBuffer, length);
            return length;
        }
        catch (Exception ex)
        {
            Log(4, $"DispatchEvent({eventType}) failed: {ex}");
            return -1;
        }
    }

    [UnmanagedCallersOnly]
    public static int ExecuteCommand(IntPtr commandNameUtf8, IntPtr senderNameUtf8, int isPlayer, IntPtr argsUtf8)
    {
        try
        {
            if (_server == null)
            {
                return -1;
            }
            string commandName = Marshal.PtrToStringUTF8(commandNameUtf8) ?? "";
            string senderName = Marshal.PtrToStringUTF8(senderNameUtf8) ?? "CONSOLE";
            string argsText = Marshal.PtrToStringUTF8(argsUtf8) ?? "";
            string[] args = string.IsNullOrWhiteSpace(argsText)
                ? Array.Empty<string>()
                : argsText.Split(' ', StringSplitOptions.RemoveEmptyEntries);

            if (!_server.Commands.TryGetValue(commandName, out Command.Command? command))
            {
                return -1;
            }

            ICommandSender sender;
            if (isPlayer != 0)
            {
                sender = _server.GetPlayer(senderName) ?? new Player.Player
                {
                    Name = senderName,
                    DisplayName = senderName,
                    IsOnline = true,
                    Server = _server,
                };
            }
            else
            {
                sender = _server.ConsoleSender;
            }

            if (!command.TestPermission(sender))
            {
                return 0;
            }
            bool result = false;
            PluginManager.RunAs(_manager.GetPluginOf(command), () => result = command.Execute(sender, commandName, args));
            return result ? 1 : 0;
        }
        catch (Exception ex)
        {
            Log(4, $"ExecuteCommand failed: {ex}");
            return 0;
        }
    }

    [UnmanagedCallersOnly]
    public static int TickScheduler(ulong tick)
    {
        try
        {
            if (_server?.Scheduler is HostTaskScheduler scheduler)
            {
                scheduler.Tick(tick);
            }
            return 0;
        }
        catch (Exception ex)
        {
            Log(4, $"TickScheduler failed: {ex}");
            return -1;
        }
    }

    [UnmanagedCallersOnly]
    public static IntPtr ListPlugins()
    {
        try
        {
            return Native.AllocatePinnedUtf8(_manager?.ListPluginsJson() ?? "[]");
        }
        catch (Exception ex)
        {
            Log(4, $"ListPlugins failed: {ex}");
            return Native.AllocatePinnedUtf8("[]");
        }
    }
}
