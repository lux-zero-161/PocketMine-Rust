pub mod config;
pub mod constants;
pub mod logger;
pub mod text;

pub use config::*;
pub use constants::*;
pub use logger::*;
pub use text::*;

/// Runtime information about the host process, comparable to PMMP's
/// `Server::getPocketMineVersion()` / `getVersion()` pair.
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub engine: &'static str,
    pub api_version: &'static str,
    pub minecraft_version: &'static str,
    pub protocol: i32,
}

impl VersionInfo {
    pub fn current() -> Self {
        Self {
            engine: VERSION,
            api_version: API_VERSION,
            minecraft_version: MINECRAFT_VERSION,
            protocol: CURRENT_PROTOCOL,
        }
    }
}
