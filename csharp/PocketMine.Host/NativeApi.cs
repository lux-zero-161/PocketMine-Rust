using System.Runtime.InteropServices;

namespace PocketMine.Host;

/// <summary>
/// C ABI table provided by the Rust core. Field order must match
/// <c>RustApiTable</c> in <c>crates/pm_plugin_host/src/lib.rs</c>.
/// </summary>
[StructLayout(LayoutKind.Sequential)]
public struct RustApiTable
{
    public IntPtr Log;
    public IntPtr BroadcastMessage;
    public IntPtr SendMessage;
    public IntPtr KickPlayer;
    public IntPtr TeleportPlayer;
    public IntPtr RegisterCommand;
    public IntPtr UnregisterCommand;
    public IntPtr DispatchCommand;
    public IntPtr GetPlayer;
    public IntPtr GetPlayers;
    public IntPtr GetWorlds;
    public IntPtr GetBlock;
    public IntPtr SetBlock;
    public IntPtr GetServerInfo;
    public IntPtr RegisterPermission;
    public IntPtr Shutdown;
    public IntPtr FreeString;
}

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void LogDelegate(int level, IntPtr message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void BroadcastMessageDelegate(IntPtr message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int SendMessageDelegate(IntPtr player, IntPtr message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int KickPlayerDelegate(IntPtr player, IntPtr reason, int isAdmin);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int TeleportPlayerDelegate(IntPtr player, double x, double y, double z);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int RegisterCommandDelegate(IntPtr name, IntPtr description, IntPtr usage, IntPtr permission, IntPtr aliases, IntPtr plugin);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int UnregisterCommandDelegate(IntPtr name);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int DispatchCommandDelegate(IntPtr sender, IntPtr command, int isPlayer);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate IntPtr GetStringDelegate(IntPtr argument);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate IntPtr GetStringEmptyDelegate();

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate IntPtr GetBlockDelegate(IntPtr world, int x, int y, int z);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int SetBlockDelegate(IntPtr world, int x, int y, int z, IntPtr name, int meta);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate int RegisterPermissionDelegate(IntPtr name, IntPtr description, IntPtr @default);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void ShutdownDelegate();

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void FreeStringDelegate(IntPtr pointer);
