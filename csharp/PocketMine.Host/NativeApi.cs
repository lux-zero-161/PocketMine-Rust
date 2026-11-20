using System.Runtime.InteropServices;

namespace PocketMine.Host;

[StructLayout(LayoutKind.Sequential)]
public struct RustApiTable
{
    public IntPtr Log;
    public IntPtr BroadcastMessage;
    public IntPtr SendMessage;
    public IntPtr KickPlayer;
    public IntPtr TeleportPlayer;
    public IntPtr RegisterCommand;
}

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void RustLogDelegate(int level, IntPtr message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void RustBroadcastMessageDelegate(IntPtr message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void RustSendMessageDelegate(IntPtr target, IntPtr message);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void RustKickPlayerDelegate(IntPtr target, IntPtr reason);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void RustTeleportPlayerDelegate(IntPtr target, double x, double y, double z);

[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
public delegate void RustRegisterCommandDelegate(IntPtr name, IntPtr description, IntPtr usage, IntPtr permission);
