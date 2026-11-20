using System.Runtime.InteropServices;
using System.Text;

namespace PocketMine.Host;

/// <summary>
/// Managed wrapper around the Rust C ABI table. Every UTF-8 string handed to
/// Rust is allocated for the duration of the call; strings coming back are
/// freed through Rust's <c>free_string</c> so allocators never mix.
/// </summary>
public static class Native
{
    private static LogDelegate? _log;
    private static BroadcastMessageDelegate? _broadcast;
    private static SendMessageDelegate? _sendMessage;
    private static KickPlayerDelegate? _kick;
    private static TeleportPlayerDelegate? _teleport;
    private static RegisterCommandDelegate? _registerCommand;
    private static UnregisterCommandDelegate? _unregisterCommand;
    private static DispatchCommandDelegate? _dispatchCommand;
    private static GetStringDelegate? _getPlayer;
    private static GetStringEmptyDelegate? _getPlayers;
    private static GetStringEmptyDelegate? _getWorlds;
    private static GetBlockDelegate? _getBlock;
    private static SetBlockDelegate? _setBlock;
    private static GetStringEmptyDelegate? _getServerInfo;
    private static RegisterPermissionDelegate? _registerPermission;
    private static ShutdownDelegate? _shutdown;
    private static FreeStringDelegate? _freeString;

    public static bool IsInitialized { get; private set; }

    public static void Initialize(RustApiTable table)
    {
        _log = Bind<LogDelegate>(table.Log);
        _broadcast = Bind<BroadcastMessageDelegate>(table.BroadcastMessage);
        _sendMessage = Bind<SendMessageDelegate>(table.SendMessage);
        _kick = Bind<KickPlayerDelegate>(table.KickPlayer);
        _teleport = Bind<TeleportPlayerDelegate>(table.TeleportPlayer);
        _registerCommand = Bind<RegisterCommandDelegate>(table.RegisterCommand);
        _unregisterCommand = Bind<UnregisterCommandDelegate>(table.UnregisterCommand);
        _dispatchCommand = Bind<DispatchCommandDelegate>(table.DispatchCommand);
        _getPlayer = Bind<GetStringDelegate>(table.GetPlayer);
        _getPlayers = Bind<GetStringEmptyDelegate>(table.GetPlayers);
        _getWorlds = Bind<GetStringEmptyDelegate>(table.GetWorlds);
        _getBlock = Bind<GetBlockDelegate>(table.GetBlock);
        _setBlock = Bind<SetBlockDelegate>(table.SetBlock);
        _getServerInfo = Bind<GetStringEmptyDelegate>(table.GetServerInfo);
        _registerPermission = Bind<RegisterPermissionDelegate>(table.RegisterPermission);
        _shutdown = Bind<ShutdownDelegate>(table.Shutdown);
        _freeString = Bind<FreeStringDelegate>(table.FreeString);
        IsInitialized = true;
    }

    private static T? Bind<T>(IntPtr pointer) where T : Delegate
        => pointer == IntPtr.Zero ? null : Marshal.GetDelegateForFunctionPointer<T>(pointer);

    /// <summary>Runs <paramref name="action"/> with a temporary UTF-8 string.</summary>
    private static T WithUtf8<T>(string value, Func<IntPtr, T> action)
    {
        IntPtr pointer = Marshal.StringToCoTaskMemUTF8(value);
        try
        {
            return action(pointer);
        }
        finally
        {
            Marshal.FreeCoTaskMem(pointer);
        }
    }

    private static T WithUtf8<T>(string a, string b, Func<IntPtr, IntPtr, T> action)
    {
        IntPtr pa = Marshal.StringToCoTaskMemUTF8(a);
        IntPtr pb = Marshal.StringToCoTaskMemUTF8(b);
        try
        {
            return action(pa, pb);
        }
        finally
        {
            Marshal.FreeCoTaskMem(pa);
            Marshal.FreeCoTaskMem(pb);
        }
    }

    private static string? ReadString(IntPtr pointer)
    {
        if (pointer == IntPtr.Zero)
        {
            return null;
        }
        try
        {
            return Marshal.PtrToStringUTF8(pointer);
        }
        finally
        {
            _freeString?.Invoke(pointer);
        }
    }

    public static void Log(int level, string message)
    {
        if (_log == null)
        {
            Console.WriteLine(message);
            return;
        }
        WithUtf8(message, p =>
        {
            _log(level, p);
            return 0;
        });
    }

    public static void Broadcast(string message)
    {
        if (_broadcast == null) return;
        WithUtf8(message, p => { _broadcast(p); return 0; });
    }

    public static bool SendMessage(string player, string message)
        => _sendMessage != null && WithUtf8(player, message, (a, b) => _sendMessage(a, b)) != 0;

    public static bool Kick(string player, string reason)
        => _kick != null && WithUtf8(player, reason, (a, b) => _kick(a, b, 0)) != 0;

    public static bool Teleport(string player, double x, double y, double z)
    {
        if (_teleport == null) return false;
        return WithUtf8(player, p => _teleport(p, x, y, z)) != 0;
    }

    public static bool RegisterCommand(string name, string description, string usage, string permission, string aliases, string plugin)
    {
        if (_registerCommand == null) return false;
        IntPtr pn = Marshal.StringToCoTaskMemUTF8(name);
        IntPtr pd = Marshal.StringToCoTaskMemUTF8(description);
        IntPtr pu = Marshal.StringToCoTaskMemUTF8(usage);
        IntPtr pp = Marshal.StringToCoTaskMemUTF8(permission);
        IntPtr pa = Marshal.StringToCoTaskMemUTF8(aliases);
        IntPtr pl = Marshal.StringToCoTaskMemUTF8(plugin);
        try
        {
            return _registerCommand(pn, pd, pu, pp, pa, pl) != 0;
        }
        finally
        {
            Marshal.FreeCoTaskMem(pn);
            Marshal.FreeCoTaskMem(pd);
            Marshal.FreeCoTaskMem(pu);
            Marshal.FreeCoTaskMem(pp);
            Marshal.FreeCoTaskMem(pa);
            Marshal.FreeCoTaskMem(pl);
        }
    }

    public static bool UnregisterCommand(string name)
        => _unregisterCommand != null && WithUtf8(name, p => _unregisterCommand(p)) != 0;

    public static bool DispatchCommand(string sender, string command, bool isPlayer)
        => _dispatchCommand != null && WithUtf8(sender, command, (a, b) => _dispatchCommand(a, b, isPlayer ? 1 : 0)) != 0;

    public static string? GetPlayer(string name)
        => _getPlayer == null ? null : ReadString(WithUtf8(name, p => _getPlayer(p)));

    public static string GetPlayers() => ReadString(_getPlayers?.Invoke() ?? IntPtr.Zero) ?? "[]";

    public static string GetWorlds() => ReadString(_getWorlds?.Invoke() ?? IntPtr.Zero) ?? "[]";

    public static string? GetBlock(string world, int x, int y, int z)
        => _getBlock == null ? null : ReadString(WithUtf8(world, p => _getBlock(p, x, y, z)));

    public static bool SetBlock(string world, int x, int y, int z, string name, int meta)
    {
        if (_setBlock == null) return false;
        IntPtr pw = Marshal.StringToCoTaskMemUTF8(world);
        IntPtr pn = Marshal.StringToCoTaskMemUTF8(name);
        try
        {
            return _setBlock(pw, x, y, z, pn, meta) != 0;
        }
        finally
        {
            Marshal.FreeCoTaskMem(pw);
            Marshal.FreeCoTaskMem(pn);
        }
    }

    public static string GetServerInfo() => ReadString(_getServerInfo?.Invoke() ?? IntPtr.Zero) ?? "{}";

    public static bool RegisterPermission(string name, string description, string @default)
    {
        if (_registerPermission == null) return false;
        IntPtr pn = Marshal.StringToCoTaskMemUTF8(name);
        IntPtr pd = Marshal.StringToCoTaskMemUTF8(description);
        IntPtr pv = Marshal.StringToCoTaskMemUTF8(@default);
        try
        {
            return _registerPermission(pn, pd, pv) != 0;
        }
        finally
        {
            Marshal.FreeCoTaskMem(pn);
            Marshal.FreeCoTaskMem(pd);
            Marshal.FreeCoTaskMem(pv);
        }
    }

    public static void Shutdown() => _shutdown?.Invoke();

    /// <summary>Allocates a NUL-terminated UTF-8 buffer that stays valid for the process.</summary>
    internal static IntPtr AllocatePinnedUtf8(string value)
    {
        byte[] bytes = Encoding.UTF8.GetBytes(value + "\0");
        GCHandle handle = GCHandle.Alloc(bytes, GCHandleType.Pinned);
        PinnedBuffers.Add(handle);
        return handle.AddrOfPinnedObject();
    }

    private static readonly List<GCHandle> PinnedBuffers = new();
}
