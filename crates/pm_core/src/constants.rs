//! Server-wide constants, kept in sync with PocketMine-MP (`src/VersionInfo.php`
//! and `vendor/nethergamesmc/bedrock-protocol/src/ProtocolInfo.php`).

/// Human readable engine name.
pub const NAME: &str = "PocketMine-Rust";
/// PocketMine-MP version this port tracks.
pub const BASE_VERSION: &str = "5.51.2";
/// Engine version string (Rust port revision appended).
pub const VERSION: &str = "1.0.0.0";
/// Plugin API version. C# plugins declare this in their manifest.
pub const API_VERSION: &str = "1.0.0.0";
/// Release channel.
pub const BUILD_CHANNEL: &str = "stable";
/// Codename used in the banner.
pub const CODENAME: &str = "RustRock";

/// Minecraft: Bedrock Edition version string (`ProtocolInfo::MINECRAFT_VERSION`).
pub const MINECRAFT_VERSION: &str = "1.26.60";
/// Version string sent over the network / in pongs.
pub const MINECRAFT_VERSION_NETWORK: &str = "1.26.60";
/// Current Minecraft network protocol version.
pub const CURRENT_PROTOCOL: i32 = 2225;

/// Protocol versions accepted for login, newest first.
pub const ACCEPTED_PROTOCOLS: &[i32] = &[
    2225, 2193, 2169, 2129, 2108, 2091, 2071, 2053, 2033, 2013, 1993, 1973, 1953, 1933, 1913, 1893,
    1873, 1853, 1833, 1813, 1793, 1773,
];

pub const DEFAULT_PORT: u16 = 19132;
pub const DEFAULT_PORT_V6: u16 = 19133;
pub const DEFAULT_QUERY_PORT: u16 = 19132;
pub const DEFAULT_MAX_PLAYERS: u32 = 20;
pub const DEFAULT_VIEW_DISTANCE: u32 = 10;
pub const DEFAULT_TICK_DISTANCE: i32 = 4;

pub const TICKS_PER_SECOND: u64 = 20;
pub const TICK_DURATION_MS: u64 = 50;

/// Maximum number of blocks in one sub chunk (16x16x16).
pub const SUB_CHUNK_BLOCK_COUNT: usize = 16 * 16 * 16;
/// World height of the Overworld in blocks.
pub const WORLD_HEIGHT: i32 = 320;
/// Lowest buildable Y coordinate.
pub const WORLD_MIN_Y: i32 = -64;
/// Highest buildable Y coordinate.
pub const WORLD_MAX_Y: i32 = WORLD_MIN_Y + WORLD_HEIGHT;
/// Number of sub chunks in a full height chunk column.
pub const WORLD_HEIGHT_SUBCHUNKS: i32 = WORLD_HEIGHT / 16;

/// Network magic shared by every RakNet implementation.
pub const RAKNET_MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

/// Directory names used by the server.
pub const DATA_DIRECTORY: &str = "data";
pub const PLUGIN_DIRECTORY: &str = "plugins";
pub const RESOURCE_PACK_DIRECTORY: &str = "resource_packs";
pub const WORLD_DIRECTORY: &str = "worlds";
pub const LOG_DIRECTORY: &str = "logs";
pub const CRASH_DIRECTORY: &str = "crashdumps";

pub fn is_protocol_accepted(protocol: i32) -> bool {
    ACCEPTED_PROTOCOLS.contains(&protocol)
}
