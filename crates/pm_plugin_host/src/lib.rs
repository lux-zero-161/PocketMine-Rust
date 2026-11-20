use pm_command::{Command, CommandSender, SimpleCommandMap};
use pm_core::{LogLevel, Logger};
use pm_event::EventTypeId;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[repr(C)]
pub struct RustApiTable {
    pub log: extern "C" fn(c_int, *const c_char),
    pub broadcast_message: extern "C" fn(*const c_char),
    pub send_message: extern "C" fn(*const c_char, *const c_char),
    pub kick_player: extern "C" fn(*const c_char, *const c_char),
    pub teleport_player: extern "C" fn(*const c_char, f64, f64, f64),
    pub register_command: extern "C" fn(*const c_char, *const c_char, *const c_char, *const c_char),
}

#[cfg(unix)]
type HostfxrChar = c_char;
#[cfg(windows)]
type HostfxrChar = u16;

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
    assembly_path_utf8: *const HostfxrChar,
    type_name_utf8: *const HostfxrChar,
    method_name_utf8: *const HostfxrChar,
    delegate_type_name_utf8: *const HostfxrChar,
    reserved: *const c_void,
    delegate: *mut *mut c_void,
) -> i32;

type InitHostFn = unsafe extern "C" fn(api_table: *const RustApiTable) -> c_int;
type LoadPluginsFn = unsafe extern "C" fn(dir_utf8: *const c_char) -> c_int;
type EnablePluginsFn = unsafe extern "C" fn() -> c_int;
type DisablePluginsFn = unsafe extern "C" fn() -> c_int;
type DispatchEventFn = unsafe extern "C" fn(event_type: c_int, json_utf8: *const c_char, cancel_out: *mut c_int) -> c_int;
type ExecuteCommandFn = unsafe extern "C" fn(cmd_utf8: *const c_char, sender_utf8: *const c_char, is_player: c_int, args_utf8: *const c_char) -> c_int;

static GLOBAL_LOGGER: OnceLock<Arc<Logger>> = OnceLock::new();
static GLOBAL_COMMAND_MAP: OnceLock<Arc<SimpleCommandMap>> = OnceLock::new();
static GLOBAL_PLUGIN_HOST: OnceLock<Arc<Mutex<Option<CSharpFunctions>>>> = OnceLock::new();

struct CSharpFunctions {
    init_host: InitHostFn,
    load_plugins: LoadPluginsFn,
    enable_plugins: EnablePluginsFn,
    disable_plugins: DisablePluginsFn,
    dispatch_event: DispatchEventFn,
    execute_command: ExecuteCommandFn,
}

extern "C" fn rust_log_callback(level: c_int, msg: *const c_char) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        let text = if msg.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(msg).to_str().unwrap_or("") }
        };
        let log_level = match level {
            0 => LogLevel::Debug,
            1 => LogLevel::Info,
            2 => LogLevel::Notice,
            3 => LogLevel::Warning,
            4 => LogLevel::Error,
            _ => LogLevel::Critical,
        };
        logger.log(log_level, text);
    }
}

extern "C" fn rust_broadcast_message_callback(msg: *const c_char) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        let text = if msg.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(msg).to_str().unwrap_or("") }
        };
        logger.info(&format!("[Broadcast] {}", text));
    }
}

extern "C" fn rust_send_message_callback(target: *const c_char, msg: *const c_char) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        let target_str = if target.is_null() { "Player" } else { unsafe { CStr::from_ptr(target).to_str().unwrap_or("Player") } };
        let msg_str = if msg.is_null() { "" } else { unsafe { CStr::from_ptr(msg).to_str().unwrap_or("") } };
        logger.info(&format!("[To {}] {}", target_str, msg_str));
    }
}

extern "C" fn rust_kick_player_callback(target: *const c_char, reason: *const c_char) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        let target_str = if target.is_null() { "Player" } else { unsafe { CStr::from_ptr(target).to_str().unwrap_or("Player") } };
        let reason_str = if reason.is_null() { "Kicked" } else { unsafe { CStr::from_ptr(reason).to_str().unwrap_or("Kicked") } };
        logger.warning(&format!("Kicked {}: {}", target_str, reason_str));
    }
}

extern "C" fn rust_teleport_player_callback(target: *const c_char, x: f64, y: f64, z: f64) {
    if let Some(logger) = GLOBAL_LOGGER.get() {
        let target_str = if target.is_null() { "Player" } else { unsafe { CStr::from_ptr(target).to_str().unwrap_or("Player") } };
        logger.info(&format!("Teleported {} to ({:.2}, {:.2}, {:.2})", target_str, x, y, z));
    }
}

struct CSharpProxyCommand {
    name: String,
    desc: String,
    usage: String,
    perm: Option<String>,
}

impl Command for CSharpProxyCommand {
    fn name(&self) -> &str {
        &self.name
    }
    fn description(&self) -> &str {
        &self.desc
    }
    fn usage(&self) -> &str {
        &self.usage
    }
    fn permission(&self) -> Option<&str> {
        self.perm.as_deref()
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let args_joined = args.join(" ");
        if let Some(host_lock) = GLOBAL_PLUGIN_HOST.get() {
            let guard = host_lock.lock().unwrap();
            if let Some(ref funcs) = *guard {
                let c_name = CString::new(self.name.as_str()).unwrap();
                let c_sender = CString::new(sender.name()).unwrap();
                let c_args = CString::new(args_joined.as_str()).unwrap();
                let is_player = if sender.is_player() { 1 } else { 0 };

                let result = unsafe {
                    (funcs.execute_command)(c_name.as_ptr(), c_sender.as_ptr(), is_player, c_args.as_ptr())
                };
                return result >= 0;
            }
        }
        false
    }
}

extern "C" fn rust_register_command_callback(
    name: *const c_char,
    desc: *const c_char,
    usage: *const c_char,
    perm: *const c_char,
) {
    if let Some(cmd_map) = GLOBAL_COMMAND_MAP.get() {
        let name_str = if name.is_null() { return; } else { unsafe { CStr::from_ptr(name).to_str().unwrap_or("").to_string() } };
        let desc_str = if desc.is_null() { String::new() } else { unsafe { CStr::from_ptr(desc).to_str().unwrap_or("").to_string() } };
        let usage_str = if usage.is_null() { String::new() } else { unsafe { CStr::from_ptr(usage).to_str().unwrap_or("").to_string() } };
        let perm_str = if perm.is_null() {
            None
        } else {
            let s = unsafe { CStr::from_ptr(perm).to_str().unwrap_or("").to_string() };
            if s.is_empty() { None } else { Some(s) }
        };

        let cmd = Arc::new(CSharpProxyCommand {
            name: name_str,
            desc: desc_str,
            usage: usage_str,
            perm: perm_str,
        });

        cmd_map.register(cmd);
    }
}

pub struct CSharpPluginManager {
    logger: Arc<Logger>,
    context_handle: *mut c_void,
    close_fn: Option<HostfxrCloseFn>,
    funcs: Option<CSharpFunctions>,
}

unsafe impl Send for CSharpPluginManager {}
unsafe impl Sync for CSharpPluginManager {}

impl CSharpPluginManager {
    pub fn new(
        logger: Arc<Logger>,
        command_map: Arc<SimpleCommandMap>,
    ) -> Result<Self, String> {
        let _ = GLOBAL_LOGGER.set(logger.clone());
        let _ = GLOBAL_COMMAND_MAP.set(command_map);
        let _ = GLOBAL_PLUGIN_HOST.set(Arc::new(Mutex::new(None)));

        let fxr_path = Self::find_hostfxr()
            .ok_or_else(|| "Could not locate libhostfxr.so/hostfxr.dll. Ensure .NET 8/9/10 SDK or runtime is installed.".to_string())?;

        logger.info(&format!("Loading .NET CoreCLR host from: {}", fxr_path.display()));

        #[cfg(unix)]
        let (_lib, init_fn_ptr, get_delegate_fn_ptr, close_fn_ptr) = unsafe {
            let c_fxr_path = CString::new(fxr_path.to_str().unwrap()).unwrap();
            let lib = libc::dlopen(c_fxr_path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
            if lib.is_null() {
                let err = CStr::from_ptr(libc::dlerror()).to_string_lossy().to_string();
                return Err(format!("dlopen failed for libhostfxr: {}", err));
            }
            (
                lib,
                libc::dlsym(lib, b"hostfxr_initialize_for_runtime_config\0".as_ptr() as *const c_char),
                libc::dlsym(lib, b"hostfxr_get_runtime_delegate\0".as_ptr() as *const c_char),
                libc::dlsym(lib, b"hostfxr_close\0".as_ptr() as *const c_char),
            )
        };

        #[cfg(windows)]
        let (_lib, init_fn_ptr, get_delegate_fn_ptr, close_fn_ptr) = unsafe {
            use std::os::windows::ffi::OsStrExt;
            let wide: Vec<u16> = fxr_path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            extern "system" {
                fn LoadLibraryW(lpLibFileName: *const u16) -> *mut c_void;
                fn GetProcAddress(hModule: *mut c_void, lpProcName: *const c_char) -> *mut c_void;
            }
            let lib = LoadLibraryW(wide.as_ptr());
            if lib.is_null() {
                return Err("LoadLibraryW failed for hostfxr.dll".to_string());
            }
            (
                lib,
                GetProcAddress(lib, b"hostfxr_initialize_for_runtime_config\0".as_ptr() as *const c_char),
                GetProcAddress(lib, b"hostfxr_get_runtime_delegate\0".as_ptr() as *const c_char),
                GetProcAddress(lib, b"hostfxr_close\0".as_ptr() as *const c_char),
            )
        };

        if init_fn_ptr.is_null() || get_delegate_fn_ptr.is_null() || close_fn_ptr.is_null() {
            return Err("Failed to resolve hostfxr exports".to_string());
        }

        let init_fn: HostfxrInitializeForRuntimeConfigFn = unsafe { std::mem::transmute(init_fn_ptr) };
        let get_delegate_fn: HostfxrGetRuntimeDelegateFn = unsafe { std::mem::transmute(get_delegate_fn_ptr) };
        let close_fn: HostfxrCloseFn = unsafe { std::mem::transmute(close_fn_ptr) };

        let candidates = [
            (
                PathBuf::from("csharp/PocketMine.Host/bin/Release/net10.0/PocketMine.Host.runtimeconfig.json"),
                PathBuf::from("csharp/PocketMine.Host/bin/Release/net10.0/PocketMine.Host.dll"),
            ),
            (
                PathBuf::from("csharp/PocketMine.Host/bin/Debug/net10.0/PocketMine.Host.runtimeconfig.json"),
                PathBuf::from("csharp/PocketMine.Host/bin/Debug/net10.0/PocketMine.Host.dll"),
            ),
        ];

        let (runtime_config, host_dll) = candidates
            .into_iter()
            .find(|(cfg, dll)| cfg.exists() && dll.exists())
            .ok_or_else(|| {
                "PocketMine.Host binaries not found in bin/Release or bin/Debug. Build C# projects with 'dotnet build' first.".to_string()
            })?;

        let abs_config = runtime_config.canonicalize().map_err(|e| e.to_string())?;
        let abs_dll = host_dll.canonicalize().map_err(|e| e.to_string())?;

        let mut context_handle: *mut c_void = std::ptr::null_mut();

        #[cfg(unix)]
        let rc = {
            let c_config = CString::new(abs_config.to_str().unwrap()).unwrap();
            unsafe { init_fn(c_config.as_ptr(), std::ptr::null(), &mut context_handle) }
        };

        #[cfg(windows)]
        let rc = {
            use std::os::windows::ffi::OsStrExt;
            let w_config: Vec<u16> = abs_config.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
            unsafe { init_fn(w_config.as_ptr(), std::ptr::null(), &mut context_handle) }
        };

        if rc != 0 || context_handle.is_null() {
            return Err(format!("hostfxr_initialize_for_runtime_config failed with code: 0x{:x}", rc));
        }

        let mut load_assembly_ptr: *mut c_void = std::ptr::null_mut();
        let rc = unsafe { get_delegate_fn(context_handle, 5 /* HDT_LOAD_ASSEMBLY_AND_GET_FUNCTION_POINTER */, &mut load_assembly_ptr) };
        if rc != 0 || load_assembly_ptr.is_null() {
            unsafe { close_fn(context_handle); }
            return Err(format!("hostfxr_get_runtime_delegate failed: 0x{:x}", rc));
        }

        let load_assembly: LoadAssemblyAndGetFunctionPointerFn = unsafe { std::mem::transmute(load_assembly_ptr) };

        let resolve_fn = |method_name: &str| -> Result<*mut c_void, String> {
            let mut ptr: *mut c_void = std::ptr::null_mut();

            #[cfg(unix)]
            let rc = {
                let c_dll = CString::new(abs_dll.to_str().unwrap()).unwrap();
                let c_type = CString::new("PocketMine.Host.Entrypoint, PocketMine.Host").unwrap();
                let c_method = CString::new(method_name).unwrap();
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
                use std::os::windows::ffi::OsStrExt;
                let w_dll: Vec<u16> = abs_dll.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
                let w_type: Vec<u16> = std::ffi::OsStr::new("PocketMine.Host.Entrypoint, PocketMine.Host").encode_wide().chain(std::iter::once(0)).collect();
                let w_method: Vec<u16> = std::ffi::OsStr::new(method_name).encode_wide().chain(std::iter::once(0)).collect();
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
                Err(format!("Failed to bind method '{}': 0x{:x}", method_name, rc))
            } else {
                Ok(ptr)
            }
        };

        let init_host: InitHostFn = unsafe { std::mem::transmute(resolve_fn("InitializeHost")?) };
        let load_plugins: LoadPluginsFn = unsafe { std::mem::transmute(resolve_fn("LoadPlugins")?) };
        let enable_plugins: EnablePluginsFn = unsafe { std::mem::transmute(resolve_fn("EnablePlugins")?) };
        let disable_plugins: DisablePluginsFn = unsafe { std::mem::transmute(resolve_fn("DisablePlugins")?) };
        let dispatch_event: DispatchEventFn = unsafe { std::mem::transmute(resolve_fn("DispatchEvent")?) };
        let execute_command: ExecuteCommandFn = unsafe { std::mem::transmute(resolve_fn("ExecuteCommand")?) };

        let funcs = CSharpFunctions {
            init_host,
            load_plugins,
            enable_plugins,
            disable_plugins,
            dispatch_event,
            execute_command,
        };

        let api_table = RustApiTable {
            log: rust_log_callback,
            broadcast_message: rust_broadcast_message_callback,
            send_message: rust_send_message_callback,
            kick_player: rust_kick_player_callback,
            teleport_player: rust_teleport_player_callback,
            register_command: rust_register_command_callback,
        };

        let rc = unsafe { (funcs.init_host)(&api_table) };
        if rc != 0 {
            unsafe { close_fn(context_handle); }
            return Err(format!("InitializeHost in C# returned error: {}", rc));
        }

        if let Some(lock) = GLOBAL_PLUGIN_HOST.get() {
            let mut guard = lock.lock().unwrap();
            *guard = Some(CSharpFunctions {
                init_host,
                load_plugins,
                enable_plugins,
                disable_plugins,
                dispatch_event,
                execute_command,
            });
        }

        Ok(Self {
            logger,
            context_handle,
            close_fn: Some(close_fn),
            funcs: Some(funcs),
        })
    }

    pub fn load_plugins(&self, plugins_dir: &str) -> usize {
        if let Some(ref funcs) = self.funcs {
            let c_dir = CString::new(plugins_dir).unwrap();
            let count = unsafe { (funcs.load_plugins)(c_dir.as_ptr()) };
            self.logger.info(&format!("Loaded {} C# plugin(s)", count));
            count as usize
        } else {
            0
        }
    }

    pub fn enable_plugins(&self) {
        if let Some(ref funcs) = self.funcs {
            unsafe { (funcs.enable_plugins)() };
        }
    }

    pub fn disable_plugins(&self) {
        if let Some(ref funcs) = self.funcs {
            unsafe { (funcs.disable_plugins)() };
        }
    }

    pub fn dispatch_event(&self, event_type: EventTypeId, json_payload: &str) -> bool {
        if let Some(ref funcs) = self.funcs {
            let c_json = match CString::new(json_payload) {
                Ok(s) => s,
                Err(_) => return false,
            };
            let mut cancel_out: c_int = 0;
            unsafe {
                (funcs.dispatch_event)(event_type as c_int, c_json.as_ptr(), &mut cancel_out);
            }
            cancel_out != 0
        } else {
            false
        }
    }

    fn find_hostfxr() -> Option<PathBuf> {
        #[cfg(unix)]
        let candidates = [
            "/usr/share/dotnet/host/fxr",
            "/usr/lib/dotnet/host/fxr",
            "/usr/local/share/dotnet/host/fxr",
        ];

        #[cfg(windows)]
        let candidates = [
            "C:\\Program Files\\dotnet\\host\\fxr",
            "C:\\Program Files (x86)\\dotnet\\host\\fxr",
        ];

        #[cfg(unix)]
        let lib_name = "libhostfxr.so";
        #[cfg(windows)]
        let lib_name = "hostfxr.dll";

        for base in &candidates {
            let p = Path::new(base);
            if p.is_dir() {
                if let Ok(entries) = std::fs::read_dir(p) {
                    for entry in entries.flatten() {
                        let candidate = entry.path().join(lib_name);
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        None
    }
}

impl Drop for CSharpPluginManager {
    fn drop(&mut self) {
        if let Some(close) = self.close_fn {
            if !self.context_handle.is_null() {
                unsafe { close(self.context_handle); }
            }
        }
    }
}
