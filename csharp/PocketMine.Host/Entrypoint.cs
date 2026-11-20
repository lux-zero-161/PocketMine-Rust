using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Math;
using PocketMine.Player;

namespace PocketMine.Host;

public static unsafe class Entrypoint
{
    private static RustLogDelegate? _logDelegate;
    private static RustBroadcastMessageDelegate? _broadcastMessageDelegate;
    private static RustSendMessageDelegate? _sendMessageDelegate;
    private static RustKickPlayerDelegate? _kickPlayerDelegate;
    private static RustTeleportPlayerDelegate? _teleportPlayerDelegate;
    private static RustRegisterCommandDelegate? _registerCommandDelegate;

    private static readonly PluginManager _pluginManager = new();

    public static void Log(int level, string message)
    {
        if (_logDelegate == null) return;
        byte[] bytes = Encoding.UTF8.GetBytes(message + "\0");
        fixed (byte* p = bytes)
        {
            _logDelegate(level, (IntPtr)p);
        }
    }

    public static void BroadcastMessage(string message)
    {
        if (_broadcastMessageDelegate == null) return;
        byte[] bytes = Encoding.UTF8.GetBytes(message + "\0");
        fixed (byte* p = bytes)
        {
            _broadcastMessageDelegate((IntPtr)p);
        }
    }

    public static void SendMessage(string target, string message)
    {
        if (_sendMessageDelegate == null) return;
        byte[] targetBytes = Encoding.UTF8.GetBytes(target + "\0");
        byte[] msgBytes = Encoding.UTF8.GetBytes(message + "\0");
        fixed (byte* t = targetBytes, m = msgBytes)
        {
            _sendMessageDelegate((IntPtr)t, (IntPtr)m);
        }
    }

    public static void KickPlayer(string target, string reason)
    {
        if (_kickPlayerDelegate == null) return;
        byte[] targetBytes = Encoding.UTF8.GetBytes(target + "\0");
        byte[] reasonBytes = Encoding.UTF8.GetBytes(reason + "\0");
        fixed (byte* t = targetBytes, r = reasonBytes)
        {
            _kickPlayerDelegate((IntPtr)t, (IntPtr)r);
        }
    }

    public static void TeleportPlayer(string target, double x, double y, double z)
    {
        if (_teleportPlayerDelegate == null) return;
        byte[] targetBytes = Encoding.UTF8.GetBytes(target + "\0");
        fixed (byte* t = targetBytes)
        {
            _teleportPlayerDelegate((IntPtr)t, x, y, z);
        }
    }

    public static void RegisterCommand(string name, string description, string usage, string? permission)
    {
        if (_registerCommandDelegate == null) return;
        byte[] n = Encoding.UTF8.GetBytes(name + "\0");
        byte[] d = Encoding.UTF8.GetBytes(description + "\0");
        byte[] u = Encoding.UTF8.GetBytes(usage + "\0");
        byte[] p = Encoding.UTF8.GetBytes((permission ?? "") + "\0");
        fixed (byte* pn = n, pd = d, pu = u, pp = p)
        {
            _registerCommandDelegate((IntPtr)pn, (IntPtr)pd, (IntPtr)pu, (IntPtr)pp);
        }
    }

    [UnmanagedCallersOnly]
    public static int InitializeHost(IntPtr apiTablePtr)
    {
        if (apiTablePtr == IntPtr.Zero) return -1;
        var api = *(RustApiTable*)apiTablePtr;

        if (api.Log != IntPtr.Zero)
            _logDelegate = Marshal.GetDelegateForFunctionPointer<RustLogDelegate>(api.Log);
        if (api.BroadcastMessage != IntPtr.Zero)
            _broadcastMessageDelegate = Marshal.GetDelegateForFunctionPointer<RustBroadcastMessageDelegate>(api.BroadcastMessage);
        if (api.SendMessage != IntPtr.Zero)
            _sendMessageDelegate = Marshal.GetDelegateForFunctionPointer<RustSendMessageDelegate>(api.SendMessage);
        if (api.KickPlayer != IntPtr.Zero)
            _kickPlayerDelegate = Marshal.GetDelegateForFunctionPointer<RustKickPlayerDelegate>(api.KickPlayer);
        if (api.TeleportPlayer != IntPtr.Zero)
            _teleportPlayerDelegate = Marshal.GetDelegateForFunctionPointer<RustTeleportPlayerDelegate>(api.TeleportPlayer);
        if (api.RegisterCommand != IntPtr.Zero)
            _registerCommandDelegate = Marshal.GetDelegateForFunctionPointer<RustRegisterCommandDelegate>(api.RegisterCommand);

        Log(1, "PocketMine C# Plugin Host initialized successfully via .NET 10 CoreCLR");
        return 0;
    }

    [UnmanagedCallersOnly]
    public static int LoadPlugins(IntPtr dirUtf8)
    {
        string dir = Marshal.PtrToStringUTF8(dirUtf8) ?? "plugins";
        _pluginManager.LoadPlugins(dir);
        return _pluginManager.Plugins.Count;
    }

    [UnmanagedCallersOnly]
    public static int EnablePlugins()
    {
        _pluginManager.EnablePlugins();
        return 0;
    }

    [UnmanagedCallersOnly]
    public static int DisablePlugins()
    {
        _pluginManager.DisablePlugins();
        return 0;
    }

    [UnmanagedCallersOnly]
    public static int DispatchEvent(int eventType, IntPtr jsonUtf8, IntPtr cancelOut)
    {
        string json = Marshal.PtrToStringUTF8(jsonUtf8) ?? "{}";
        Event.Event? ev = null;

        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;

            switch (eventType)
            {
                case 1: // PlayerJoin
                {
                    string playerName = root.GetProperty("player_name").GetString() ?? "Player";
                    string joinMessage = root.GetProperty("join_message").GetString() ?? "";
                    var player = new Player.Player(playerName, Guid.NewGuid().ToString(), Vector3.Zero, SendMessage, KickPlayer, TeleportPlayer);
                    ev = new PlayerJoinEvent(player, joinMessage);
                    break;
                }
                case 2: // PlayerQuit
                {
                    string playerName = root.GetProperty("player_name").GetString() ?? "Player";
                    string quitMessage = root.GetProperty("quit_message").GetString() ?? "";
                    var player = new Player.Player(playerName, Guid.NewGuid().ToString(), Vector3.Zero, SendMessage, KickPlayer, TeleportPlayer);
                    ev = new PlayerQuitEvent(player, quitMessage);
                    break;
                }
                case 3: // PlayerChat
                {
                    string playerName = root.GetProperty("player_name").GetString() ?? "Player";
                    string message = root.GetProperty("message").GetString() ?? "";
                    string format = root.GetProperty("format").GetString() ?? "<{0}> {1}";
                    var player = new Player.Player(playerName, Guid.NewGuid().ToString(), Vector3.Zero, SendMessage, KickPlayer, TeleportPlayer);
                    ev = new PlayerChatEvent(player, message, format);
                    break;
                }
                case 4: // BlockBreak
                {
                    string playerName = root.GetProperty("player_name").GetString() ?? "Player";
                    uint blockId = root.GetProperty("block_id").GetUInt32();
                    var posElem = root.GetProperty("block_pos");
                    var pos = new Vector3(posElem.GetProperty("x").GetDouble(), posElem.GetProperty("y").GetDouble(), posElem.GetProperty("z").GetDouble());
                    var player = new Player.Player(playerName, Guid.NewGuid().ToString(), Vector3.Zero, SendMessage, KickPlayer, TeleportPlayer);
                    ev = new BlockBreakEvent(player, pos, blockId);
                    break;
                }
                case 5: // BlockPlace
                {
                    string playerName = root.GetProperty("player_name").GetString() ?? "Player";
                    uint blockId = root.GetProperty("block_id").GetUInt32();
                    var posElem = root.GetProperty("block_pos");
                    var pos = new Vector3(posElem.GetProperty("x").GetDouble(), posElem.GetProperty("y").GetDouble(), posElem.GetProperty("z").GetDouble());
                    var player = new Player.Player(playerName, Guid.NewGuid().ToString(), Vector3.Zero, SendMessage, KickPlayer, TeleportPlayer);
                    ev = new BlockPlaceEvent(player, pos, blockId);
                    break;
                }
            }

            if (ev != null)
            {
                _pluginManager.DispatchEvent(ev);
                if (cancelOut != IntPtr.Zero && ev.IsCancelled)
                {
                    *(int*)cancelOut = 1;
                }
            }
        }
        catch (Exception ex)
        {
            Log(4, $"Error in DispatchEvent: {ex.Message}");
            return -1;
        }

        return 0;
    }

    private class GenericConsoleSender : ICommandSender
    {
        public string Name => "CONSOLE";
        public bool IsPlayer => false;
        public void SendMessage(string message) => Log(1, message);
        public bool HasPermission(string permission) => true;
    }

    [UnmanagedCallersOnly]
    public static int ExecuteCommand(IntPtr commandNameUtf8, IntPtr senderNameUtf8, int isPlayer, IntPtr argsUtf8)
    {
        string cmdName = Marshal.PtrToStringUTF8(commandNameUtf8) ?? "";
        string senderName = Marshal.PtrToStringUTF8(senderNameUtf8) ?? "CONSOLE";
        string argsStr = Marshal.PtrToStringUTF8(argsUtf8) ?? "";

        string[] args = string.IsNullOrWhiteSpace(argsStr)
            ? Array.Empty<string>()
            : argsStr.Split(' ', StringSplitOptions.RemoveEmptyEntries);

        if (_pluginManager.Server.Commands.TryGetValue(cmdName, out var command))
        {
            ICommandSender sender = isPlayer != 0
                ? new Player.Player(senderName, Guid.NewGuid().ToString(), Vector3.Zero, SendMessage, KickPlayer, TeleportPlayer)
                : new GenericConsoleSender();

            bool result = command.Execute(sender, cmdName, args);
            return result ? 1 : 0;
        }

        return -1; // Command not found
    }
}
