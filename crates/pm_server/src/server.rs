use pm_command::{Command, ConsoleCommandSender, SimpleCommandMap};
use pm_core::{text, Logger, ServerProperties, VersionInfo};
use pm_event::{Event, PlayerJoinEvent, PlayerQuitEvent};
use pm_permission::PermissionRegistry;
use pm_player::{Player, PlayerManager};
use pm_plugin_host::{PluginHost, ServerBridge};
use pm_scheduler::TaskScheduler;
use pm_world::{Block, World, WorldManager};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Instant;

/// Shared server state. This is the single owner of worlds, players, commands
/// and the plugin host; every subsystem gets an `Arc` to it.
pub struct ServerState {
    pub logger: Arc<Logger>,
    pub props: Arc<ServerProperties>,
    pub worlds: Arc<WorldManager>,
    pub players: Arc<PlayerManager>,
    pub commands: Arc<SimpleCommandMap>,
    pub permissions: Arc<PermissionRegistry>,
    pub console: Arc<ConsoleCommandSender>,
    pub scheduler: Mutex<TaskScheduler>,
    pub running: AtomicBool,
    pub tick: AtomicU64,
    pub started: Instant,
    pub tick_times: Mutex<VecDeque<f64>>,
    pub last_tick_ms: Mutex<f64>,
    pub plugin_host: OnceLock<Arc<PluginHost>>,
    pub network: Mutex<Option<Arc<pm_network::BedrockNetworkServer>>>,
    pub worlds_path: RwLock<String>,
    /// Connected player name (lowercase) -> UDP session address.
    pub player_sessions: RwLock<HashMap<String, SocketAddr>>,
    /// UDP session address -> player name.
    pub session_players: RwLock<HashMap<SocketAddr, String>>,
    /// Per-player block palette, kept stable for the whole session.
    palettes: RwLock<HashMap<String, Arc<crate::protocol::BlockPalette>>>,
}

impl ServerState {
    pub fn new(logger: Arc<Logger>, props: Arc<ServerProperties>) -> Arc<Self> {
        let console = Arc::new(ConsoleCommandSender::new(logger.clone()));
        Arc::new(Self {
            logger,
            props,
            worlds: Arc::new(WorldManager::new()),
            players: Arc::new(PlayerManager::new()),
            commands: Arc::new(SimpleCommandMap::new()),
            permissions: Arc::new(PermissionRegistry::new()),
            console,
            scheduler: Mutex::new(TaskScheduler::new()),
            running: AtomicBool::new(true),
            tick: AtomicU64::new(0),
            started: Instant::now(),
            tick_times: Mutex::new(VecDeque::with_capacity(20)),
            last_tick_ms: Mutex::new(50.0),
            plugin_host: OnceLock::new(),
            network: Mutex::new(None),
            worlds_path: RwLock::new(pm_core::WORLD_DIRECTORY.to_string()),
            player_sessions: RwLock::new(HashMap::new()),
            session_players: RwLock::new(HashMap::new()),
            palettes: RwLock::new(HashMap::new()),
        })
    }

    pub fn tick_now(&self) -> u64 {
        self.tick.load(Ordering::Relaxed)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn register_command(&self, command: Arc<dyn Command>) {
        self.commands.register(command);
    }

    pub fn default_world(&self) -> Option<Arc<World>> {
        self.worlds.default_world()
    }

    pub fn world_or_default(&self, name: &str) -> Option<Arc<World>> {
        if name.is_empty() {
            self.default_world()
        } else {
            self.worlds.get(name)
        }
    }

    /// Sends an event to plugins and returns whether it was cancelled.
    pub fn dispatch<E: Event>(&self, event: &mut E) -> bool {
        match self.plugin_host.get() {
            Some(host) => host.dispatch_event(event),
            None => false,
        }
    }

    #[allow(dead_code)]
    pub fn dispatch_name(&self, name: &str, payload: &str) -> bool {
        match self.plugin_host.get() {
            Some(host) => host.dispatch_event_name(name, payload),
            None => false,
        }
    }

    pub fn plugin_names(&self) -> Vec<String> {
        let Some(host) = self.plugin_host.get() else {
            return Vec::new();
        };
        let json_text = host.plugin_list_json();
        let parsed: serde_json::Value = serde_json::from_str(&json_text).unwrap_or(json!([]));
        parsed
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|p| {
                        p.get("name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn average_tps(&self) -> f64 {
        let times = self.tick_times.lock().unwrap();
        if times.is_empty() {
            return 20.0;
        }
        let avg_ms: f64 = times.iter().sum::<f64>() / times.len() as f64;
        if avg_ms <= 0.0 {
            20.0
        } else {
            (1000.0 / avg_ms).min(20.0)
        }
    }

    pub fn record_tick(&self, elapsed_ms: f64) {
        *self.last_tick_ms.lock().unwrap() = elapsed_ms;
        let mut times = self.tick_times.lock().unwrap();
        if times.len() == 20 {
            times.pop_front();
        }
        times.push_back(elapsed_ms.max(0.001));
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    /// Creates the default world from the configuration and loads spawn chunks.
    pub fn prepare_default_world(&self) {
        let name = self.props.level_name.clone();
        let world = Arc::new(World::flat(name.clone()));
        world.load_chunks_around(world.spawn, 2);
        self.worlds.register(world.clone(), true);
        self.logger.info(&format!(
            "Prepared level \"{}\" ({} chunks loaded, flat generator, seed {})",
            name,
            world.loaded_chunk_count(),
            world.seed
        ));
    }

    /// Creates a fake player for testing events/commands without a client.
    pub fn spawn_test_player(&self, name: &str) -> Arc<Player> {
        if let Some(existing) = self.players.get(name) {
            return existing;
        }
        let world = self.default_world();
        let spawn = world
            .map(|w| w.spawn_position())
            .unwrap_or_else(|| pm_math::Vector3::new(0.0, 5.0, 0.0));
        let mut player = Player::new(
            name,
            format!("00000000-0000-0000-0000-{:012x}", name.len()),
            spawn,
            self.props.level_name.clone(),
        );
        player.entity_id = self.players.count() as u64 + 1;
        let player = Arc::new(player);
        self.players.add(player.clone());
        player
    }

    /// Sends queued player messages as Bedrock `Text` packets.
    pub fn flush_player_messages(&self) {
        let network = self.network.lock().unwrap().clone();
        let sessions = self.player_sessions.read().unwrap().clone();
        for player in self.players.online() {
            for message in player.pop_outbox() {
                let plain = if message.starts_with('\u{0}') {
                    text::strip_colors(&message.replace('\u{0}', " | "))
                } else {
                    message.clone()
                };
                match (
                    network.as_ref(),
                    sessions.get(&player.name.to_ascii_lowercase()),
                ) {
                    (Some(network), Some(address)) => {
                        network.send_packets(
                            *address,
                            vec![crate::protocol::text_packet(&plain, None)],
                            None,
                        );
                    }
                    _ => self
                        .logger
                        .info(&format!("[{}] {}", player.name, text::to_ansi(&plain))),
                }
            }
        }
    }

    fn broadcast_text(&self, line: &str, source: Option<&str>) {
        let network = self.network.lock().unwrap().clone();
        let sessions = self.player_sessions.read().unwrap().clone();
        if let Some(network) = network {
            for player in self.players.online() {
                if let Some(address) = sessions.get(&player.name.to_ascii_lowercase()) {
                    network.send_packets(
                        *address,
                        vec![crate::protocol::text_packet(line, source)],
                        None,
                    );
                }
            }
        }
        self.logger.info(line);
    }

    /// Turns network login/disconnect events into real player sessions and
    /// server events. Called once per tick after `network.process_packets()`.
    pub fn process_network_events(&self) {
        let network = { self.network.lock().unwrap().clone() };

        if let Some(network) = network.as_ref() {
            for login in network.take_logins() {
                let name = if login.identity.display_name.trim().is_empty() {
                    format!("Player{}", login.address.port())
                } else {
                    login.identity.display_name.clone()
                };
                let uuid = if login.identity.identity.trim().is_empty() {
                    let mut hash: u64 = 0xcbf29ce484222325;
                    for byte in name.as_bytes() {
                        hash ^= *byte as u64;
                        hash = hash.wrapping_mul(0x100000001b3);
                    }
                    format!("00000000-0000-0000-0000-{:012x}", hash & 0xffffffffffff)
                } else {
                    login.identity.identity.clone()
                };

                if let Some(existing) = self.players.get(&name) {
                    existing.set_online(false);
                    self.player_sessions
                        .write()
                        .unwrap()
                        .remove(&name.to_ascii_lowercase());
                }

                let world = self.default_world();
                let spawn = world
                    .as_ref()
                    .map(|world| world.spawn_position())
                    .unwrap_or_else(|| pm_math::Vector3::new(0.0, 5.0, 0.0));
                let mut player = Player::new(
                    name.clone(),
                    uuid.clone(),
                    spawn,
                    self.props.level_name.clone(),
                );
                player.entity_id = self.tick_now() + 1;
                player.address = login.address.ip().to_string();
                player.port = login.address.port();
                player.xuid = login.identity.xuid.clone();
                let player = Arc::new(player);
                self.players.add(player.clone());
                self.player_sessions
                    .write()
                    .unwrap()
                    .insert(name.to_ascii_lowercase(), login.address);
                self.session_players
                    .write()
                    .unwrap()
                    .insert(login.address, name.clone());

                if let Some(world) = world.as_ref() {
                    // Bounded preload; proper view-distance streaming comes with StartGame.
                    world.load_chunks_around(spawn, self.props.view_distance.min(4) as i32);
                }

                let mut event =
                    PlayerJoinEvent::new(player.name.clone(), player.uuid.clone(), String::new());
                self.dispatch(&mut event);
                let message = if event.join_message.is_empty() {
                    format!("§e{} joined the game", player.name)
                } else {
                    event.join_message.clone()
                };
                self.broadcast(&message);
                player.send_message(&format!("§aWelcome to {}!", self.props.motd));
                self.logger.info(&format!(
                    "Player {} connected from {} ({})",
                    player.name, login.address, player.uuid
                ));
            }

            for disconnect in network.take_disconnects() {
                self.player_sessions
                    .write()
                    .unwrap()
                    .remove(&disconnect.name.to_ascii_lowercase());
                self.session_players
                    .write()
                    .unwrap()
                    .remove(&disconnect.address);
                if let Some(player) = self.players.get(&disconnect.name) {
                    let mut event = PlayerQuitEvent::new(
                        player.name.clone(),
                        player.uuid.clone(),
                        String::new(),
                    );
                    self.dispatch(&mut event);
                    let message = if event.quit_message.is_empty() {
                        format!("§e{} left the game", player.name)
                    } else {
                        event.quit_message.clone()
                    };
                    self.broadcast(&message);
                }
                self.players.remove(&disconnect.name);
            }

            for address in network.take_start_games() {
                self.start_game_for(address);
            }
            for (address, radius) in network.take_chunk_requests() {
                self.send_chunks_for(address, radius);
            }

            for action in network.take_block_actions() {
                self.handle_block_action(action);
            }
            for place in network.take_block_places() {
                self.handle_block_place(place);
            }

            for (address, message) in network.take_chats() {
                let Some(name) = self.session_players.read().unwrap().get(&address).cloned() else {
                    continue;
                };
                let Some(player) = self.players.get(&name) else {
                    continue;
                };
                let mut event = pm_event::PlayerChatEvent::new(
                    player.name.clone(),
                    player.uuid.clone(),
                    message,
                    "<{0}> {1}".to_string(),
                );
                let cancelled = self.dispatch(&mut event);
                if !cancelled {
                    let format = if event.format.is_empty() {
                        "<{0}> {1}".to_string()
                    } else {
                        event.format.clone()
                    };
                    let line = format
                        .replace("{0}", &player.name)
                        .replace("{1}", &event.message);
                    self.broadcast_text(&line, None);
                }
            }

            for (address, command) in network.take_commands() {
                let Some(name) = self.session_players.read().unwrap().get(&address).cloned() else {
                    continue;
                };
                let Some(player) = self.players.get(&name) else {
                    continue;
                };
                let mut event = pm_event::PlayerCommandPreprocessEvent::new(
                    player.name.clone(),
                    format!("/{}", command),
                );
                if !self.dispatch(&mut event) {
                    let command = event.message.trim_start_matches('/');
                    self.commands.execute(&*player, command);
                }
            }
        }
    }

    /// Sends the pre-spawn packet set and the initial chunk burst.
    fn start_game_for(&self, address: SocketAddr) {
        let Some(name) = self.session_players.read().unwrap().get(&address).cloned() else {
            return;
        };
        let Some(player) = self.players.get(&name) else {
            return;
        };
        let Some(world) = self.default_world() else {
            return;
        };

        let palette = Arc::new(crate::protocol::BlockPalette::build(&world));
        self.palettes
            .write()
            .unwrap()
            .insert(player.name.to_ascii_lowercase(), palette.clone());
        let packets = crate::protocol::pre_spawn_packets(
            &player,
            &world,
            self.tick_now(),
            &self.props.motd,
            &palette,
        );
        if let Some(network) = self.network.lock().unwrap().clone() {
            network.send_packets(address, packets, None);
        }
        self.logger.info(&format!(
            "Sent StartGame to {} ({} block states in palette)",
            player.name,
            palette.len()
        ));
        self.send_chunks_for(address, 2);
        if let Some(network) = self.network.lock().unwrap().clone() {
            network.send_packets(
                address,
                crate::protocol::spawn_packets(&player, &world, self.tick_now()),
                None,
            );
        }
    }

    fn palette_for(&self, player_name: &str, world: &World) -> Arc<crate::protocol::BlockPalette> {
        if let Some(palette) = self
            .palettes
            .read()
            .unwrap()
            .get(&player_name.to_ascii_lowercase())
            .cloned()
        {
            return palette;
        }
        let palette = Arc::new(crate::protocol::BlockPalette::build(world));
        self.palettes
            .write()
            .unwrap()
            .insert(player_name.to_ascii_lowercase(), palette.clone());
        palette
    }

    fn handle_block_action(&self, action: pm_network::PendingBlockAction) {
        const STOP_BREAK: i32 = 2;
        const CREATIVE_DESTROY: i32 = 13;
        const PREDICT_DESTROY: i32 = 26;
        if !matches!(
            action.action,
            STOP_BREAK | CREATIVE_DESTROY | PREDICT_DESTROY
        ) {
            return;
        }
        let Some(name) = self
            .session_players
            .read()
            .unwrap()
            .get(&action.address)
            .cloned()
        else {
            return;
        };
        let Some(player) = self.players.get(&name) else {
            return;
        };
        let Some(world) = self.default_world() else {
            return;
        };

        let position = pm_math::BlockPosition::new(action.x, action.y, action.z);
        let block = world.get_block(&position);
        if block.is_air() {
            return;
        }
        let mut event = pm_event::BlockBreakEvent::new(
            player.name.clone(),
            position,
            block.id as u32,
            block.name().to_string(),
        );
        if self.dispatch(&mut event) {
            // Cancel: restore the block for every client.
            self.broadcast_block_update(&position, block);
            return;
        }
        world.set_block(&position, pm_world::Block::air());
        self.broadcast_block_update(&position, pm_world::Block::air());
        self.logger.info(&format!(
            "{} broke {} at {}",
            player.name,
            block.name(),
            position
        ));
    }

    fn handle_block_place(&self, place: pm_network::PendingBlockPlace) {
        let Some(name) = self
            .session_players
            .read()
            .unwrap()
            .get(&place.address)
            .cloned()
        else {
            return;
        };
        let Some(player) = self.players.get(&name) else {
            return;
        };
        let Some(world) = self.default_world() else {
            return;
        };

        let held = player.inventory().held_item().clone();
        let block_name = if place.item_id > 0 {
            pm_data::item_by_id(place.item_id)
                .map(|item| item.string_id.clone())
                .unwrap_or_else(|| held.name.clone())
        } else {
            held.name.clone()
        };
        let Some(block) = pm_world::Block::from_name(&block_name) else {
            return;
        };
        if block.is_air() {
            return;
        }

        let clicked = pm_math::BlockPosition::new(place.x, place.y, place.z);
        let target = if world.get_block(&clicked).is_replaceable() {
            clicked
        } else {
            let facing = match place.face {
                0 => pm_math::Facing::Down,
                1 => pm_math::Facing::Up,
                2 => pm_math::Facing::North,
                3 => pm_math::Facing::South,
                4 => pm_math::Facing::West,
                _ => pm_math::Facing::East,
            };
            clicked.side(facing)
        };
        if !world.get_block(&target).is_replaceable() {
            return;
        }

        let mut event = pm_event::BlockPlaceEvent::new(
            player.name.clone(),
            target,
            block.id as u32,
            block.name().to_string(),
        );
        if self.dispatch(&mut event) {
            self.broadcast_block_update(&target, world.get_block(&target));
            return;
        }

        world.set_block(&target, block);
        self.broadcast_block_update(&target, block);
        self.logger.info(&format!(
            "{} placed {} at {}",
            player.name,
            block.name(),
            target
        ));
    }

    fn broadcast_block_update(&self, position: &pm_math::BlockPosition, block: pm_world::Block) {
        let network = self.network.lock().unwrap().clone();
        let Some(network) = network else { return };
        let Some(world) = self.default_world() else {
            return;
        };
        let sessions = self.player_sessions.read().unwrap().clone();
        for player in self.players.online() {
            let Some(address) = sessions.get(&player.name.to_ascii_lowercase()) else {
                continue;
            };
            let palette = self.palette_for(&player.name, &world);
            let runtime_id = palette.id(&block);
            network.send_packets(
                *address,
                vec![crate::protocol::update_block_packet(
                    *position, runtime_id, 3, 0,
                )],
                None,
            );
        }
    }

    /// Sends `LevelChunk` packets around the player's current position.
    fn send_chunks_for(&self, address: SocketAddr, radius: i32) {
        let Some(name) = self.session_players.read().unwrap().get(&address).cloned() else {
            return;
        };
        let Some(player) = self.players.get(&name) else {
            return;
        };
        let Some(world) = self.default_world() else {
            return;
        };

        let radius = radius.clamp(2, 8);
        let palette = crate::protocol::BlockPalette::build(&world);
        let chunks =
            crate::protocol::chunk_burst(&world, player.position().floor(), radius, &palette);
        let packet_count = chunks.len();
        if let Some(network) = self.network.lock().unwrap().clone() {
            for chunk in chunks {
                network.send_packets(address, vec![chunk], None);
            }
        }
        self.logger.info(&format!(
            "Sent {} chunk(s) to {} (radius {})",
            packet_count, player.name, radius
        ));
    }

    pub fn server_info(&self) -> serde_json::Value {
        let version = VersionInfo::current();
        json!({
            "name": pm_core::NAME,
            "version": version.engine,
            "base_version": pm_core::BASE_VERSION,
            "api_version": version.api_version,
            "minecraft_version": version.minecraft_version,
            "protocol": version.protocol,
            "tick": self.tick_now(),
            "tps": self.average_tps(),
            "uptime": self.uptime_seconds(),
            "online": self.players.count(),
            "max_players": self.props.max_players,
            "worlds": self.worlds.names(),
            "plugins": self.plugin_names(),
            "motd": self.props.motd,
        })
    }

    pub fn save_worlds(&self) {
        let path = self.worlds_path.read().unwrap().clone();
        for world in self.worlds.all() {
            let dir = std::path::Path::new(&path).join(&world.name);
            if let Err(e) = world.save(&dir) {
                self.logger
                    .warning(&format!("Failed to save world {}: {}", world.name, e));
            }
        }
    }

    /// Broadcasts a message to console and every online player.
    pub fn broadcast(&self, message: &str) {
        self.logger.info(message);
        self.players.broadcast(message);
    }
}

impl ServerBridge for ServerState {
    fn log(&self, level: i32, message: &str) {
        self.logger.log(pm_core::LogLevel::from_int(level), message);
    }

    fn broadcast(&self, message: &str) {
        ServerState::broadcast(self, message);
    }

    fn send_message(&self, player: &str, message: &str) -> bool {
        match self.players.get(player) {
            Some(p) => {
                p.send_message(message);
                true
            }
            None => false,
        }
    }

    fn kick(&self, player: &str, reason: &str) -> bool {
        match self.players.get(player) {
            Some(p) => {
                p.send_message(&format!("§cKicked: {}", reason));
                p.set_online(false);
                self.logger.info(&format!("Kicked {}: {}", p.name, reason));
                true
            }
            None => false,
        }
    }

    fn teleport(&self, player: &str, x: f64, y: f64, z: f64) -> bool {
        match self.players.get(player) {
            Some(p) => {
                p.set_position(pm_math::Vector3::new(x, y, z));
                true
            }
            None => false,
        }
    }

    fn player_json(&self, name: &str) -> Option<String> {
        self.players.get(name).map(|p| p.to_json().to_string())
    }

    fn players_json(&self) -> String {
        json!(self
            .players
            .online()
            .iter()
            .map(|p| p.to_json())
            .collect::<Vec<_>>())
        .to_string()
    }

    fn worlds_json(&self) -> String {
        let list: Vec<serde_json::Value> = self
            .worlds
            .all()
            .iter()
            .map(|w| {
                json!({
                    "name": w.name,
                    "spawn": w.spawn,
                    "time": w.time(),
                    "chunks": w.loaded_chunk_count(),
                    "entities": w.loaded_entity_count(),
                    "generator": w.generator.name(),
                    "difficulty": w.difficulty as u8,
                    "ticking": w.ticking,
                })
            })
            .collect();
        json!(list).to_string()
    }

    fn block_json(&self, world: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let world = self.world_or_default(world)?;
        let block = world.get_block_at(x, y, z);
        Some(
            json!({
                "name": block.name(),
                "id": block.id,
                "meta": block.meta,
                "hardness": block.hardness(),
                "blast_resistance": block.blast_resistance(),
                "light": block.light_level(),
                "solid": block.is_solid(),
                "air": block.is_air(),
            })
            .to_string(),
        )
    }

    fn set_block(&self, world: &str, x: i32, y: i32, z: i32, name: &str, meta: u16) -> bool {
        let Some(world) = self.world_or_default(world) else {
            return false;
        };
        let Some(block) = Block::from_name(name) else {
            return false;
        };
        world.set_block_at(x, y, z, Block { meta, ..block })
    }

    fn server_info_json(&self) -> String {
        self.server_info().to_string()
    }

    fn dispatch_command(&self, sender: &str, command: &str) -> bool {
        if sender.eq_ignore_ascii_case("console") || sender.eq_ignore_ascii_case("server") {
            return self.commands.execute(&*self.console, command);
        }
        match self.players.get(sender) {
            Some(player) => self.commands.execute(&*player, command),
            None => false,
        }
    }

    fn shutdown(&self) {
        ServerState::shutdown(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_network::{decode_frames, SplitAssembly};
    use pm_player::Player;
    use std::net::UdpSocket;
    use std::time::Duration;

    fn raknet_client(server: &pm_network::BedrockNetworkServer) -> (UdpSocket, SocketAddr) {
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        let session_addr = client.local_addr().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let size: libc::c_int = 8 * 1024 * 1024;
            unsafe {
                libc::setsockopt(
                    client.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_RCVBUF,
                    &size as *const _ as *const libc::c_void,
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
            }
        }
        let addr = server.local_addr().unwrap();
        let mut buf = [0u8; 4096];

        let mut request = vec![0x05];
        request.extend_from_slice(&pm_core::RAKNET_MAGIC);
        request.push(11);
        request.extend_from_slice(&[0u8; 1300]);
        client.send_to(&request, addr).unwrap();
        server.process_packets();
        let (len, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(buf[0], 0x06);
        let mtu = u16::from_be_bytes([buf[len - 2], buf[len - 1]]);

        let mut request = vec![0x07];
        request.extend_from_slice(&pm_core::RAKNET_MAGIC);
        request.extend_from_slice(&[4, 127, 0, 0, 1, 0x4f, 0x9f]);
        request.extend_from_slice(&mtu.to_be_bytes());
        request.extend_from_slice(&7i64.to_be_bytes());
        client.send_to(&request, addr).unwrap();
        server.process_packets();
        let (_, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(buf[0], 0x08);
        (client, session_addr)
    }

    fn recv_batch(client: &UdpSocket) -> Vec<Vec<u8>> {
        let mut assemblies: HashMap<u16, SplitAssembly> = HashMap::new();
        let mut buf = [0u8; 4096];
        loop {
            let (len, _) = client.recv_from(&mut buf).unwrap();
            if buf[0] != 0x84 {
                continue;
            }
            for frame in decode_frames(&buf[4..len]) {
                let payload = if frame.is_split() {
                    let assembly = assemblies
                        .entry(frame.split_id)
                        .or_insert_with(|| SplitAssembly::new(frame.split_count));
                    let result = assembly.push(frame.split_index, frame.payload);
                    if result.is_some() {
                        assemblies.remove(&frame.split_id);
                    }
                    result
                } else {
                    Some(frame.payload)
                };
                if let Some(payload) = payload {
                    return pm_network::decode_batch(&payload).expect("batch");
                }
            }
        }
    }

    fn packet_id(packet: &[u8]) -> u32 {
        let mut offset = 0usize;
        pm_network::read_varint(packet, &mut offset).unwrap()
    }

    #[test]
    fn start_game_and_chunks_are_sent_to_the_session() {
        let logger = Arc::new(Logger::new("test"));
        let props = Arc::new(pm_core::ServerProperties::default());
        let state = ServerState::new(logger, props);
        state.prepare_default_world();

        let network = Arc::new(
            pm_network::BedrockNetworkServer::bind("127.0.0.1:0", "Test", 10, state.logger.clone())
                .unwrap(),
        );
        *state.network.lock().unwrap() = Some(network.clone());

        let (client, addr) = raknet_client(&network);
        let world = state.default_world().unwrap();
        let player = Arc::new(Player::new(
            "Steve",
            "uuid",
            world.spawn_position(),
            world.name.clone(),
        ));
        state.players.add(player.clone());
        state
            .session_players
            .write()
            .unwrap()
            .insert(addr, "Steve".to_string());

        state.start_game_for(addr);

        let pre_spawn = recv_batch(&client);
        assert_eq!(
            packet_id(&pre_spawn[0]),
            crate::protocol::JIGSAW_STRUCTURE_DATA_PACKET
        );
        assert_eq!(
            packet_id(&pre_spawn[1]),
            crate::protocol::VOXEL_SHAPES_PACKET
        );
        assert_eq!(packet_id(&pre_spawn[2]), crate::protocol::START_GAME_PACKET);
        assert_eq!(
            packet_id(&pre_spawn[3]),
            crate::protocol::ITEM_REGISTRY_PACKET
        );
        assert_eq!(
            packet_id(&pre_spawn[4]),
            crate::protocol::AVAILABLE_ACTOR_IDENTIFIERS_PACKET
        );
        assert_eq!(
            packet_id(&pre_spawn[5]),
            crate::protocol::BIOME_DEFINITION_LIST_PACKET
        );

        let chunk = recv_batch(&client);
        assert_eq!(packet_id(&chunk[0]), crate::protocol::LEVEL_CHUNK_PACKET);
    }
}

#[cfg(test)]
mod block_tests {
    use super::*;
    use pm_network::{PendingBlockAction, PendingBlockPlace};
    use pm_player::{ItemStack, Player};
    use pm_world::blocks;
    use std::net::{Ipv4Addr, SocketAddr};

    #[test]
    fn breaks_and_places_blocks_in_the_world() {
        let logger = Arc::new(Logger::new("test"));
        let state = ServerState::new(logger, Arc::new(pm_core::ServerProperties::default()));
        state.prepare_default_world();
        let world = state.default_world().unwrap();

        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 4242));
        let player = Arc::new(Player::new(
            "Steve",
            "uuid",
            pm_math::Vector3::new(0.5, 5.0, 0.5),
            world.name.clone(),
        ));
        player
            .inventory_mut()
            .set(0, ItemStack::new("minecraft:stone", 1));
        state.players.add(player);
        state
            .session_players
            .write()
            .unwrap()
            .insert(address, "Steve".to_string());

        world.set_block_at(0, 3, 0, blocks::grass());
        state.handle_block_action(PendingBlockAction {
            address,
            action: 13,
            x: 0,
            y: 3,
            z: 0,
            face: 1,
        });
        assert!(world.get_block_at(0, 3, 0).is_air());
        assert_eq!(world.get_block_at(0, 2, 0).name(), "minecraft:dirt");

        state.handle_block_place(PendingBlockPlace {
            address,
            x: 0,
            y: 2,
            z: 0,
            face: 1,
            item_id: 1,
            item_count: 1,
        });
        assert_eq!(world.get_block_at(0, 3, 0).name(), "minecraft:stone");
    }
}
