//! In-process C# plugin runtime.
//!
//! The Rust server hosts the real .NET runtime (`hostfxr`) and calls managed
//! entry points through `[UnmanagedCallersOnly]`. No sockets, no serialising
//! side-channel: plugin events are plain C ABI calls with UTF-8 JSON payloads.
//!
//! The layout is intentionally split in two:
//!   * [`ServerBridge`] – implemented by `pm_server`; lets plugins query and
//!     mutate live server state (players, worlds, blocks, commands).
//!   * [`PluginHost`] – owns the CoreCLR context and the managed function table.

use pm_command::{Command, CommandSender, SimpleCommandMap};
use pm_core::{LogLevel, Logger};
use pm_event::{Event, EventTypeId};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

// ---------------------------------------------------------------------------
// Bridge implemented by the server
// ---------------------------------------------------------------------------

/// Everything a C# plugin is allowed to do to the live server.
pub trait ServerBridge: Send + Sync {
    fn log(&self, level: i32, message: &str);
    fn broadcast(&self, message: &str);
    fn send_message(&self, player: &str, message: &str) -> bool;
    fn kick(&self, player: &str, reason: &str) -> bool;
    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool;
    fn player_json(&self, name: &str) -> Option<String>;
    fn players_json(&self) -> String;
    fn worlds_json(&self) -> String;
    fn block_json(&self, world: &str, x: i32, y: i32, z: i32) -> Option<String>;
    fn set_block(&self, world: &str, x: i32, y: i32, z: i32, name: &str, meta: u16) -> bool;
    fn server_info_json(&self) -> String;
    fn dispatch_command(&self, sender: &str, command: &str) -> bool;
    fn shutdown(&self);
    /// Called by C# when it wants a message mirrored to the server log.
    fn request_tick(&self) {}
}

// ---------------------------------------------------------------------------
// FFI table
// ---------------------------------------------------------------------------

pub type LogFn = extern "C" fn(c_int, *const c_char);
pub type BroadcastFn = extern "C" fn(*const c_char);
pub type SendMessageFn = extern "C" fn(*const c_char, *const c_char) -> c_int;
pub type KickFn = extern "C" fn(*const c_char, *const c_char, c_int) -> c_int;
pub type TeleportFn = extern "C" fn(*const c_char, f64, f64, f64) -> c_int;
pub type RegisterCommandFn = extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
) -> c_int;
pub type UnregisterCommandFn = extern "C" fn(*const c_char) -> c_int;
pub type DispatchCommandFn = extern "C" fn(*const c_char, *const c_char, c_int) -> c_int;
pub type GetStringFn = extern "C" fn(*const c_char) -> *mut c_char;
pub type GetStringEmptyFn = extern "C" fn() -> *mut c_char;
pub type GetBlockFn = extern "C" fn(*const c_char, c_int, c_int, c_int) -> *mut c_char;
pub type SetBlockFn =
    extern "C" fn(*const c_char, c_int, c_int, c_int, *const c_char, c_int) -> c_int;
pub type FreeStringFn = extern "C" fn(*mut c_char);
pub type ShutdownFn = extern "C" fn();
pub type RegisterPermissionFn = extern "C" fn(*const c_char, *const c_char, *const c_char) -> c_int;

/// C ABI surface handed to `PocketMine.Host`. Field order must match
/// `RustApiTable` in `csharp/PocketMine.Host/NativeApi.cs`.
#[repr(C)]
pub struct RustApiTable {
    pub log: LogFn,
    pub broadcast_message: BroadcastFn,
    pub send_message: SendMessageFn,
    pub kick_player: KickFn,
    pub teleport_player: TeleportFn,
    pub register_command: RegisterCommandFn,
    pub unregister_command: UnregisterCommandFn,
    pub dispatch_command: DispatchCommandFn,
    pub get_player: GetStringFn,
    pub get_players: GetStringEmptyFn,
    pub get_worlds: GetStringEmptyFn,
    pub get_block: GetBlockFn,
    pub set_block: SetBlockFn,
    pub get_server_info: GetStringEmptyFn,
    pub register_permission: RegisterPermissionFn,
    pub shutdown: ShutdownFn,
    pub free_string: FreeStringFn,
}

#[cfg(unix)]
type HostfxrChar = c_char;
#[cfg(windows)]
type HostfxrChar = u16;

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

type HostfxrInitializeForRuntimeConfigFn = unsafe extern "C" fn(
    runtime_config_path: *const HostfxrChar,
    parameters: *const c_void,
    host_context_handle: *mut *mut c_void,
) -> i32;
type HostfxrGetRuntimeDelegateFn = unsafe extern "C" fn(
    host_context_handle: *mut c_void,
    r#type: c_int,
    delegate: *mut *mut c_void,
) -> i32;
type HostfxrCloseFn = unsafe extern "C" fn(host_context_handle: *mut c_void) -> i32;
type LoadAssemblyAndGetFunctionPointerFn = unsafe extern "C" fn(
    assembly_path: *const HostfxrChar,
    type_name: *const HostfxrChar,
    method_name: *const HostfxrChar,
    delegate_type_name: *const HostfxrChar,
    reserved: *const c_void,
    delegate: *mut *mut c_void,
) -> i32;

type InitHostFn = unsafe extern "C" fn(api_table: *const RustApiTable) -> c_int;
type LoadPluginsFn = unsafe extern "C" fn(dir_utf8: *const c_char) -> c_int;
type VoidFn = unsafe extern "C" fn() -> c_int;
type DispatchEventFn = unsafe extern "C" fn(
    event_type: c_int,
    json: *const c_char,
    cancel_out: *mut c_int,
    result_buf: *mut c_char,
    result_capacity: c_int,
) -> c_int;
type ExecuteCommandFn =
    unsafe extern "C" fn(*const c_char, *const c_char, c_int, *const c_char) -> c_int;
type TickSchedulerFn = unsafe extern "C" fn(tick: u64) -> c_int;
type ListPluginsFn = unsafe extern "C" fn() -> *mut c_char;

#[derive(Clone, Copy)]
struct ManagedFunctions {
    init_host: InitHostFn,
    load_plugins: LoadPluginsFn,
    enable_plugins: VoidFn,
    disable_plugins: VoidFn,
    dispatch_event: DispatchEventFn,
    execute_command: ExecuteCommandFn,
    tick_scheduler: TickSchedulerFn,
    list_plugins: ListPluginsFn,
}

// ---------------------------------------------------------------------------
// Globals used by the C callbacks
// ---------------------------------------------------------------------------

static GLOBAL_LOGGER: OnceLock<Arc<Logger>> = OnceLock::new();
static GLOBAL_COMMAND_MAP: OnceLock<Arc<SimpleCommandMap>> = OnceLock::new();
static GLOBAL_BRIDGE: OnceLock<Arc<dyn ServerBridge>> = OnceLock::new();
static GLOBAL_PERMISSIONS: OnceLock<Arc<pm_permission::PermissionRegistry>> = OnceLock::new();
static GLOBAL_HOST: OnceLock<Arc<HostHandle>> = OnceLock::new();

struct HostHandle {
    functions: Mutex<Option<ManagedFunctions>>,
}

fn with_bridge<F: FnOnce(&dyn ServerBridge) -> R, R>(fallback: R, f: F) -> R {
    match GLOBAL_BRIDGE.get() {
        Some(bridge) => f(bridge.as_ref()),
        None => fallback,
    }
}

fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(s) => s.into_raw(),
        Err(_) => CString::new("").unwrap().into_raw(),
    }
}

extern "C" fn cb_log(level: c_int, message: *const c_char) {
    let text = cstr(message);
    if let Some(logger) = GLOBAL_LOGGER.get() {
        logger.log(LogLevel::from_int(level), &text);
    } else {
        println!("{}", text);
    }
}

extern "C" fn cb_broadcast(message: *const c_char) {
    let text = cstr(message);
    with_bridge((), |bridge| bridge.broadcast(&text));
}

extern "C" fn cb_send_message(target: *const c_char, message: *const c_char) -> c_int {
    let target = cstr(target);
    let message = cstr(message);
    with_bridge(false, |bridge| bridge.send_message(&target, &message)) as c_int
}

extern "C" fn cb_kick(target: *const c_char, reason: *const c_char, _is_admin: c_int) -> c_int {
    let target = cstr(target);
    let reason = cstr(reason);
    with_bridge(false, |bridge| bridge.kick(&target, &reason)) as c_int
}

extern "C" fn cb_teleport(target: *const c_char, x: f64, y: f64, z: f64) -> c_int {
    let target = cstr(target);
    with_bridge(false, |bridge| bridge.teleport(&target, x, y, z)) as c_int
}

struct PluginCommand {
    name: String,
    description: String,
    usage: String,
    permission: Option<String>,
    aliases: Vec<String>,
    plugin: String,
}

impl Command for PluginCommand {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.description
    }
    fn usage(&self) -> &str {
        &self.usage
    }
    fn aliases(&self) -> &[String] {
        &self.aliases
    }
    fn permission(&self) -> Option<&str> {
        self.permission.as_deref()
    }
    fn plugin(&self) -> Option<&str> {
        Some(&self.plugin)
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let Some(host) = GLOBAL_HOST.get() else {
            return false;
        };
        let functions = { host.functions.lock().unwrap().as_ref().copied() };
        let Some(functions) = functions else {
            return false;
        };
        let Ok(name) = CString::new(self.name.as_str()) else {
            return false;
        };
        let Ok(sender_name) = CString::new(sender.name()) else {
            return false;
        };
        let Ok(args_joined) = CString::new(args.join(" ")) else {
            return false;
        };
        let is_player = sender.is_player() as c_int;
        let result = unsafe {
            (functions.execute_command)(
                name.as_ptr(),
                sender_name.as_ptr(),
                is_player,
                args_joined.as_ptr(),
            )
        };
        result > 0
    }
}

extern "C" fn cb_register_command(
    name: *const c_char,
    description: *const c_char,
    usage: *const c_char,
    permission: *const c_char,
    aliases: *const c_char,
    plugin: *const c_char,
) -> c_int {
    let Some(command_map) = GLOBAL_COMMAND_MAP.get() else {
        return 0;
    };
    let name = cstr(name);
    if name.is_empty() {
        return 0;
    }
    let permission = cstr(permission);
    let aliases = cstr(aliases)
        .split(',')
        .map(str::trim)
        .filter(|a| !a.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let command = Arc::new(PluginCommand {
        name,
        description: cstr(description),
        usage: cstr(usage),
        permission: if permission.is_empty() {
            None
        } else {
            Some(permission)
        },
        aliases,
        plugin: cstr(plugin),
    });
    command_map.register(command);
    1
}

extern "C" fn cb_unregister_command(name: *const c_char) -> c_int {
    let Some(command_map) = GLOBAL_COMMAND_MAP.get() else {
        return 0;
    };
    command_map.unregister(&cstr(name)) as c_int
}

extern "C" fn cb_dispatch_command(
    sender: *const c_char,
    command: *const c_char,
    _is_player: c_int,
) -> c_int {
    let sender = cstr(sender);
    let command = cstr(command);
    with_bridge(false, |bridge| bridge.dispatch_command(&sender, &command)) as c_int
}

extern "C" fn cb_get_player(name: *const c_char) -> *mut c_char {
    let name = cstr(name);
    with_bridge(None, |bridge| bridge.player_json(&name))
        .map(to_c_string)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn cb_get_players() -> *mut c_char {
    to_c_string(with_bridge("[]".to_string(), |bridge| {
        bridge.players_json()
    }))
}

extern "C" fn cb_get_worlds() -> *mut c_char {
    to_c_string(with_bridge("[]".to_string(), |bridge| bridge.worlds_json()))
}

extern "C" fn cb_get_block(world: *const c_char, x: c_int, y: c_int, z: c_int) -> *mut c_char {
    let world = cstr(world);
    with_bridge(None, |bridge| bridge.block_json(&world, x, y, z))
        .map(to_c_string)
        .unwrap_or(std::ptr::null_mut())
}

extern "C" fn cb_set_block(
    world: *const c_char,
    x: c_int,
    y: c_int,
    z: c_int,
    name: *const c_char,
    meta: c_int,
) -> c_int {
    let world = cstr(world);
    let name = cstr(name);
    with_bridge(false, |bridge| {
        bridge.set_block(&world, x, y, z, &name, meta as u16)
    }) as c_int
}

extern "C" fn cb_get_server_info() -> *mut c_char {
    to_c_string(with_bridge("{}".to_string(), |bridge| {
        bridge.server_info_json()
    }))
}

extern "C" fn cb_register_permission(
    name: *const c_char,
    description: *const c_char,
    default: *const c_char,
) -> c_int {
    let Some(registry) = GLOBAL_PERMISSIONS.get() else {
        return 0;
    };
    let name = cstr(name);
    if name.is_empty() {
        return 0;
    }
    registry.register(pm_permission::Permission::new(
        name,
        cstr(description),
        pm_permission::PermissionDefault::parse(&cstr(default)),
    ));
    1
}

extern "C" fn cb_shutdown() {
    with_bridge((), |bridge| bridge.shutdown());
}

extern "C" fn cb_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}

// ---------------------------------------------------------------------------
// Host
// ---------------------------------------------------------------------------

pub struct PluginHost {
    logger: Arc<Logger>,
    context_handle: *mut c_void,
    close_fn: Option<HostfxrCloseFn>,
    handle: Arc<HostHandle>,
    loaded: AtomicBool,
}

unsafe impl Send for PluginHost {}
unsafe impl Sync for PluginHost {}

impl PluginHost {
    pub fn new(
        logger: Arc<Logger>,
        command_map: Arc<SimpleCommandMap>,
        permissions: Arc<pm_permission::PermissionRegistry>,
        bridge: Arc<dyn ServerBridge>,
    ) -> Result<Arc<Self>, String> {
        let _ = GLOBAL_LOGGER.set(logger.clone());
        let _ = GLOBAL_COMMAND_MAP.set(command_map);
        let _ = GLOBAL_PERMISSIONS.set(permissions);
        let _ = GLOBAL_BRIDGE.set(bridge);

        let fxr_path = Self::find_hostfxr().ok_or_else(|| {
            "libhostfxr not found. Install the .NET SDK/runtime (8, 9 or 10) or set DOTNET_ROOT.".to_string()
        })?;
        logger.info(&format!("CoreCLR host: {}", fxr_path.display()));

        let lib = Self::load_library(&fxr_path)?;
        let init_fn: HostfxrInitializeForRuntimeConfigFn = unsafe {
            std::mem::transmute(Self::sym(lib, "hostfxr_initialize_for_runtime_config")?)
        };
        let get_delegate_fn: HostfxrGetRuntimeDelegateFn =
            unsafe { std::mem::transmute(Self::sym(lib, "hostfxr_get_runtime_delegate")?) };
        let close_fn: HostfxrCloseFn =
            unsafe { std::mem::transmute(Self::sym(lib, "hostfxr_close")?) };

        let (runtime_config, host_dll) = Self::find_host_assembly()?;
        logger.info(&format!("Plugin host assembly: {}", host_dll.display()));

        let abs_config = runtime_config.canonicalize().map_err(|e| e.to_string())?;
        let abs_dll = host_dll.canonicalize().map_err(|e| e.to_string())?;

        let mut context_handle: *mut c_void = std::ptr::null_mut();

        #[cfg(unix)]
        let rc = {
            let c_config =
                CString::new(abs_config.to_string_lossy().as_ref()).map_err(|e| e.to_string())?;
            unsafe { init_fn(c_config.as_ptr(), std::ptr::null(), &mut context_handle) }
        };
        #[cfg(windows)]
        let rc = {
            let w_config = wide(abs_config.to_string_lossy().as_ref());
            unsafe { init_fn(w_config.as_ptr(), std::ptr::null(), &mut context_handle) }
        };
        if rc != 0 || context_handle.is_null() {
            return Err(format!(
                "hostfxr_initialize_for_runtime_config failed: 0x{rc:x}"
            ));
        }

        let mut load_ptr: *mut c_void = std::ptr::null_mut();
        let rc = unsafe { get_delegate_fn(context_handle, 5, &mut load_ptr) };
        if rc != 0 || load_ptr.is_null() {
            unsafe { close_fn(context_handle) };
            return Err(format!("hostfxr_get_runtime_delegate failed: 0x{rc:x}"));
        }
        let load_assembly: LoadAssemblyAndGetFunctionPointerFn =
            unsafe { std::mem::transmute(load_ptr) };

        let resolve = |method: &str| -> Result<*mut c_void, String> {
            let mut ptr: *mut c_void = std::ptr::null_mut();

            #[cfg(unix)]
            let rc = {
                let c_dll =
                    CString::new(abs_dll.to_string_lossy().as_ref()).map_err(|e| e.to_string())?;
                let c_type = CString::new("PocketMine.Host.Entrypoint, PocketMine.Host").unwrap();
                let c_method = CString::new(method).map_err(|e| e.to_string())?;
                unsafe {
                    load_assembly(
                        c_dll.as_ptr(),
                        c_type.as_ptr(),
                        c_method.as_ptr(),
                        -1isize as *const c_char, // UNMANAGEDCALLERSONLY_METHOD
                        std::ptr::null(),
                        &mut ptr,
                    )
                }
            };

            #[cfg(windows)]
            let rc = {
                let w_dll = wide(abs_dll.to_string_lossy().as_ref());
                let w_type = wide("PocketMine.Host.Entrypoint, PocketMine.Host");
                let w_method = wide(method);
                unsafe {
                    load_assembly(
                        w_dll.as_ptr(),
                        w_type.as_ptr(),
                        w_method.as_ptr(),
                        -1isize as *const u16,
                        std::ptr::null(),
                        &mut ptr,
                    )
                }
            };

            if rc != 0 || ptr.is_null() {
                Err(format!("failed to bind {method}: 0x{rc:x}"))
            } else {
                Ok(ptr)
            }
        };

        let functions = ManagedFunctions {
            init_host: unsafe { std::mem::transmute(resolve("InitializeHost")?) },
            load_plugins: unsafe { std::mem::transmute(resolve("LoadPlugins")?) },
            enable_plugins: unsafe { std::mem::transmute(resolve("EnablePlugins")?) },
            disable_plugins: unsafe { std::mem::transmute(resolve("DisablePlugins")?) },
            dispatch_event: unsafe { std::mem::transmute(resolve("DispatchEvent")?) },
            execute_command: unsafe { std::mem::transmute(resolve("ExecuteCommand")?) },
            tick_scheduler: unsafe { std::mem::transmute(resolve("TickScheduler")?) },
            list_plugins: unsafe { std::mem::transmute(resolve("ListPlugins")?) },
        };

        let api_table = RustApiTable {
            log: cb_log,
            broadcast_message: cb_broadcast,
            send_message: cb_send_message,
            kick_player: cb_kick,
            teleport_player: cb_teleport,
            register_command: cb_register_command,
            unregister_command: cb_unregister_command,
            dispatch_command: cb_dispatch_command,
            get_player: cb_get_player,
            get_players: cb_get_players,
            get_worlds: cb_get_worlds,
            get_block: cb_get_block,
            set_block: cb_set_block,
            get_server_info: cb_get_server_info,
            register_permission: cb_register_permission,
            shutdown: cb_shutdown,
            free_string: cb_free_string,
        };

        let rc = unsafe { (functions.init_host)(&api_table) };
        if rc != 0 {
            unsafe { close_fn(context_handle) };
            return Err(format!("C# InitializeHost returned {rc}"));
        }

        let handle = Arc::new(HostHandle {
            functions: Mutex::new(Some(functions)),
        });
        let _ = GLOBAL_HOST.set(handle.clone());

        Ok(Arc::new(Self {
            logger,
            context_handle,
            close_fn: Some(close_fn),
            handle,
            loaded: AtomicBool::new(false),
        }))
    }

    /// Loads plugin assemblies from `directory`. Returns the number of plugins.
    pub fn load_plugins(&self, directory: impl AsRef<Path>) -> usize {
        let dir = directory.as_ref();
        let _ = std::fs::create_dir_all(dir);
        let abs = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let Ok(c_dir) = CString::new(abs.to_string_lossy().as_ref()) else {
            return 0;
        };
        let functions = { self.handle.functions.lock().unwrap().as_ref().copied() };
        let Some(functions) = functions else { return 0 };
        let count = unsafe { (functions.load_plugins)(c_dir.as_ptr()) };
        count.max(0) as usize
    }

    pub fn enable_plugins(&self) {
        if let Some(functions) = { self.handle.functions.lock().unwrap().as_ref().copied() } {
            unsafe { (functions.enable_plugins)() };
        }
    }

    pub fn disable_plugins(&self) {
        if let Some(functions) = { self.handle.functions.lock().unwrap().as_ref().copied() } {
            unsafe { (functions.disable_plugins)() };
        }
    }

    /// Advances C# scheduler tasks. Called from the server tick.
    pub fn tick_scheduler(&self, tick: u64) {
        if let Some(functions) = { self.handle.functions.lock().unwrap().as_ref().copied() } {
            unsafe { (functions.tick_scheduler)(tick) };
        }
    }

    pub fn plugin_list_json(&self) -> String {
        let functions = { self.handle.functions.lock().unwrap().as_ref().copied() };
        let Some(functions) = functions else {
            return "[]".to_string();
        };
        let ptr = unsafe { (functions.list_plugins)() };
        if ptr.is_null() {
            return "[]".to_string();
        }
        // The managed side keeps this buffer pinned for the process lifetime,
        // so it must not be freed with Rust's allocator here.
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }

    /// Dispatches a typed event to every C# listener. Returns `true` when a
    /// listener cancelled the event.
    pub fn dispatch_event(&self, event: &mut dyn Event) -> bool {
        let Some(id) = EventTypeId::from_name(event.name()) else {
            return false;
        };
        let original = event.to_json();
        let payload = match CString::new(original.to_string()) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let functions = { self.handle.functions.lock().unwrap().as_ref().copied() };
        let Some(functions) = functions else {
            return false;
        };

        let mut cancelled: c_int = event.is_cancelled() as c_int;
        let mut buffer = vec![0u8; 64 * 1024];
        let written = unsafe {
            (functions.dispatch_event)(
                id as c_int,
                payload.as_ptr(),
                &mut cancelled,
                buffer.as_mut_ptr() as *mut c_char,
                buffer.len() as c_int,
            )
        };

        if written > 0 && (written as usize) <= buffer.len() {
            if let Ok(text) = std::str::from_utf8(&buffer[..written as usize]) {
                if let Ok(patch) = serde_json::from_str::<serde_json::Value>(text) {
                    event.apply_patch(&patch);
                }
            }
        }
        event.set_cancelled(cancelled != 0);
        cancelled != 0
    }

    /// Dispatch by raw name, used by the network/command layers.
    pub fn dispatch_event_name(&self, name: &str, json: &str) -> bool {
        let Some(id) = EventTypeId::from_name(name) else {
            return false;
        };
        let Ok(payload) = CString::new(json) else {
            return false;
        };
        let functions = { self.handle.functions.lock().unwrap().as_ref().copied() };
        let Some(functions) = functions else {
            return false;
        };
        let mut cancelled: c_int = 0;
        let mut buffer = vec![0u8; 1024];
        unsafe {
            (functions.dispatch_event)(
                id as c_int,
                payload.as_ptr(),
                &mut cancelled,
                buffer.as_mut_ptr() as *mut c_char,
                buffer.len() as c_int,
            );
        }
        cancelled != 0
    }

    fn find_host_assembly() -> Result<(PathBuf, PathBuf), String> {
        let candidates = [
            "csharp/PocketMine.Host/bin/Release/net10.0",
            "csharp/PocketMine.Host/bin/Debug/net10.0",
            "PocketMine.Host",
        ];
        for dir in candidates {
            let dir = PathBuf::from(dir);
            let config = dir.join("PocketMine.Host.runtimeconfig.json");
            let dll = dir.join("PocketMine.Host.dll");
            if config.exists() && dll.exists() {
                return Ok((config, dll));
            }
        }
        Err("PocketMine.Host not built. Run ./build.sh (needs the .NET 10 SDK).".to_string())
    }

    fn find_hostfxr() -> Option<PathBuf> {
        if let Ok(root) = std::env::var("DOTNET_ROOT") {
            if let Some(path) = Self::scan_fxr(Path::new(&root)) {
                return Some(path);
            }
        }
        let candidates = [
            "/usr/share/dotnet",
            "/usr/lib/dotnet",
            "/usr/local/share/dotnet",
            "/opt/dotnet",
            "C:\\Program Files\\dotnet",
        ];
        for base in candidates {
            if let Some(path) = Self::scan_fxr(Path::new(base)) {
                return Some(path);
            }
        }
        None
    }

    fn scan_fxr(dotnet_root: &Path) -> Option<PathBuf> {
        let lib_name = if cfg!(windows) {
            "hostfxr.dll"
        } else if cfg!(target_os = "macos") {
            "libhostfxr.dylib"
        } else {
            "libhostfxr.so"
        };
        let fxr = dotnet_root.join("host").join("fxr");
        let mut versions: Vec<PathBuf> = std::fs::read_dir(fxr)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .collect();
        // Highest version first.
        versions.sort();
        versions.reverse();
        for version in versions {
            let candidate = version.join(lib_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    #[cfg(unix)]
    fn load_library(path: &Path) -> Result<*mut c_void, String> {
        let c_path = CString::new(path.to_string_lossy().as_ref()).map_err(|e| e.to_string())?;
        let handle = unsafe { libc::dlopen(c_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL) };
        if handle.is_null() {
            let err = unsafe { CStr::from_ptr(libc::dlerror()) }
                .to_string_lossy()
                .into_owned();
            return Err(format!("dlopen({}) failed: {err}", path.display()));
        }
        Ok(handle)
    }

    #[cfg(windows)]
    fn load_library(path: &Path) -> Result<*mut c_void, String> {
        use std::os::windows::ffi::OsStrExt;
        extern "system" {
            fn LoadLibraryW(name: *const u16) -> *mut c_void;
        }
        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let handle = unsafe { LoadLibraryW(wide.as_ptr()) };
        if handle.is_null() {
            Err(format!("LoadLibraryW({}) failed", path.display()))
        } else {
            Ok(handle)
        }
    }

    #[cfg(unix)]
    fn sym(lib: *mut c_void, name: &str) -> Result<*mut c_void, String> {
        let c_name = CString::new(name).map_err(|e| e.to_string())?;
        let ptr = unsafe { libc::dlsym(lib, c_name.as_ptr()) };
        if ptr.is_null() {
            Err(format!("missing hostfxr export: {name}"))
        } else {
            Ok(ptr)
        }
    }

    #[cfg(windows)]
    fn sym(lib: *mut c_void, name: &str) -> Result<*mut c_void, String> {
        extern "system" {
            fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
        }
        let c_name = CString::new(name).map_err(|e| e.to_string())?;
        let ptr = unsafe { GetProcAddress(lib, c_name.as_ptr()) };
        if ptr.is_null() {
            Err(format!("missing hostfxr export: {name}"))
        } else {
            Ok(ptr)
        }
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        if self.loaded.load(Ordering::Relaxed) {
            self.disable_plugins();
        }
        if let Some(close) = self.close_fn {
            if !self.context_handle.is_null() {
                unsafe { close(self.context_handle) };
            }
        }
        let _ = self.logger;
    }
}

/// Convenience alias kept for the existing server bootstrap code.
pub type CSharpPluginManager = PluginHost;
