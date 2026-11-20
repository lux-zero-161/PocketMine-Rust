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
        let _ = world.save(std::path::Path::new(&format!(
            "{}/{}",
            pm_core::WORLD_DIRECTORY,
            name
        )));
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

    /// Flushes queued player messages. Until the chat packet encoder lands the
    /// console mirrors them so plugin output is visible.
    pub fn flush_player_messages(&self) {
        for player in self.players.online() {
            for message in player.pop_outbox() {
                if message.starts_with('\u{0}') {
                    let plain = text::strip_colors(&message.replace('\u{0}', " | "));
                    self.logger.info(&format!(
                        "[{}] {}",
                        player.name,
                        plain.trim_start_matches(" | ")
                    ));
                } else {
                    self.logger
                        .info(&format!("[{}] {}", player.name, text::to_ansi(&message)));
                }
            }
        }
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
                    self.player_sessions.write().unwrap().remove(&name.to_ascii_lowercase());
                }

                let world = self.default_world();
                let spawn = world
                    .as_ref()
                    .map(|world| world.spawn_position())
                    .unwrap_or_else(|| pm_math::Vector3::new(0.0, 5.0, 0.0));
                let mut player = Player::new(name.clone(), uuid.clone(), spawn, self.props.level_name.clone());
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

                if let Some(world) = world.as_ref() {
                    // Bounded preload; proper view-distance streaming comes with StartGame.
                    world.load_chunks_around(spawn, self.props.view_distance.min(4) as i32);
                }

                let mut event = PlayerJoinEvent::new(player.name.clone(), player.uuid.clone(), String::new());
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
                if let Some(player) = self.players.get(&disconnect.name) {
                    let mut event = PlayerQuitEvent::new(player.name.clone(), player.uuid.clone(), String::new());
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
        }
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
