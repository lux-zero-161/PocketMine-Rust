use crate::player_commands::{GamemodeCommand, GiveCommand, KillCommand, TeleportCommand};
use crate::server::ServerState;
use pm_command::{Command, CommandSender};
use pm_core::{text, VersionInfo};
use pm_event::{
    BlockBreakEvent, BlockPlaceEvent, EntityDamageEvent, PlayerChatEvent, PlayerDeathEvent,
    PlayerJoinEvent, PlayerQuitEvent, PlayerRespawnEvent, ServerStartEvent, ServerStopEvent,
};
use pm_math::BlockPosition;
use pm_world::Block;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct HelpCommand {
    state: Arc<ServerState>,
}

impl HelpCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }
    fn description(&self) -> &str {
        "Shows the command list"
    }
    fn usage(&self) -> &str {
        "/help [command]"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        if let Some(name) = args.first() {
            match self.state.commands.get_command(name) {
                Some(command) => {
                    sender.send_message(&format!(
                        "§b{}§7: {}",
                        command.name(),
                        command.description()
                    ));
                    sender.send_message(&format!("§7Usage: §f{}", command.usage()));
                    let aliases = self.state.commands.aliases_of(command.name());
                    if !aliases.is_empty() {
                        sender.send_message(&format!("§7Aliases: §f{}", aliases.join(", ")));
                    }
                }
                None => sender.send_message(&format!("§cNo command named '{}'", name)),
            }
            return true;
        }

        let mut names = self.state.commands.get_all_names();
        names.sort();
        sender.send_message("§2--- PocketMine-Rust commands ---");
        for name in names {
            if let Some(command) = self.state.commands.get_command(&name) {
                sender.send_message(&format!("§a/{:<12} §7{}", name, command.description()));
            }
        }
        sender.send_message("§7Type §f/help <command>§7 for details.");
        true
    }
}

pub struct StopCommand {
    state: Arc<ServerState>,
}

impl StopCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for StopCommand {
    fn name(&self) -> &str {
        "stop"
    }
    fn description(&self) -> &str {
        "Stops the server"
    }
    fn usage(&self) -> &str {
        "/stop"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        sender.send_message("§eStopping the server...");
        self.state
            .logger
            .info("Stopping server (requested via /stop)");
        request_stop(&self.state);
        true
    }
}

pub struct VersionCommand {
    state: Arc<ServerState>,
}

impl VersionCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for VersionCommand {
    fn name(&self) -> &str {
        "version"
    }
    fn description(&self) -> &str {
        "Shows engine and protocol versions"
    }
    fn usage(&self) -> &str {
        "/version"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        let v = VersionInfo::current();
        sender.send_message(&format!("§b{} §7({})", pm_core::NAME, v.engine));
        sender.send_message(&format!(
            "§7PocketMine-MP base: §f{}",
            pm_core::BASE_VERSION
        ));
        sender.send_message(&format!("§7Plugin API: §f{}", v.api_version));
        sender.send_message(&format!(
            "§7Minecraft: §f{} §7(protocol §f{}§7)",
            v.minecraft_version, v.protocol
        ));
        sender.send_message(&format!("§7Uptime: §f{}s", self.state.uptime_seconds()));
        true
    }
}

pub struct SayCommand {
    state: Arc<ServerState>,
}

impl SayCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for SayCommand {
    fn name(&self) -> &str {
        "say"
    }
    fn description(&self) -> &str {
        "Broadcasts a message"
    }
    fn usage(&self) -> &str {
        "/say <message>"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, _sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        if args.is_empty() {
            return false;
        }
        self.state
            .broadcast(&format!("§d[SERVER] §f{}", args.join(" ")));
        true
    }
}

pub struct ListCommand {
    state: Arc<ServerState>,
}

impl ListCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for ListCommand {
    fn name(&self) -> &str {
        "list"
    }
    fn description(&self) -> &str {
        "Lists online players"
    }
    fn usage(&self) -> &str {
        "/list"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        let names = self.state.players.names();
        sender.send_message(&format!(
            "§7Online players (§f{}§7/§f{}§7): §a{}",
            names.len(),
            self.state.props.max_players,
            if names.is_empty() {
                "none".to_string()
            } else {
                names.join("§7, §a")
            }
        ));
        true
    }
}

pub struct PluginsCommand {
    state: Arc<ServerState>,
}

impl PluginsCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for PluginsCommand {
    fn name(&self) -> &str {
        "plugins"
    }
    fn description(&self) -> &str {
        "Lists loaded plugins"
    }
    fn usage(&self) -> &str {
        "/plugins"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        let plugins = self.state.plugin_names();
        if plugins.is_empty() {
            sender.send_message("§7No plugins loaded.");
        } else {
            sender.send_message(&format!(
                "§7Plugins (§f{}§7): §a{}",
                plugins.len(),
                plugins.join("§7, §a")
            ));
        }
        true
    }
}

pub struct TpsCommand {
    state: Arc<ServerState>,
}

impl TpsCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for TpsCommand {
    fn name(&self) -> &str {
        "tps"
    }
    fn description(&self) -> &str {
        "Shows tick performance"
    }
    fn usage(&self) -> &str {
        "/tps"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        sender.send_message(&format!(
            "§7TPS: §a{:.2}§7, last tick: §f{:.2}ms§7, tick §f{}",
            self.state.average_tps(),
            *self.state.last_tick_ms.lock().unwrap(),
            self.state.tick_now()
        ));
        true
    }
}

pub struct StatusCommand {
    state: Arc<ServerState>,
}

impl StatusCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for StatusCommand {
    fn name(&self) -> &str {
        "status"
    }
    fn description(&self) -> &str {
        "Shows world and plugin status"
    }
    fn usage(&self) -> &str {
        "/status"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        sender.send_message("§2--- Server status ---");
        for world in self.state.worlds.all() {
            sender.send_message(&format!(
                "§7World §f{}§7: {} chunks, {} entities, time {}",
                world.name,
                world.loaded_chunk_count(),
                world.loaded_entity_count(),
                world.time()
            ));
        }
        sender.send_message(&format!(
            "§7Players: §f{}§7, tasks: §f{}",
            self.state.players.count(),
            self.state.scheduler.lock().unwrap().pending()
        ));
        if let Some(network) = self.state.network.lock().unwrap().as_ref() {
            let stats = network.stats();
            sender.send_message(&format!(
                "§7Network: §f{}§7 sessions, §f{}§7 pings, §f{}§7 handshakes, §f{}§7 encrypted",
                network.session_count(),
                stats.pings,
                stats.handshakes,
                stats.encryptions
            ));
        }
        true
    }
}

pub struct WorldCommand {
    state: Arc<ServerState>,
}

impl WorldCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for WorldCommand {
    fn name(&self) -> &str {
        "world"
    }
    fn description(&self) -> &str {
        "World, time and block operations"
    }
    fn usage(&self) -> &str {
        "/world <info|list|time|settime|getblock|setblock|tp> ..."
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let Some(sub) = args.first().map(|s| s.to_ascii_lowercase()) else {
            sender.send_message(self.usage());
            return false;
        };
        match sub.as_str() {
            "list" => {
                sender.send_message(&format!(
                    "§7Worlds: §a{}",
                    self.state.worlds.names().join("§7, §a")
                ));
                true
            }
            "info" => {
                let name = args.get(1).copied().unwrap_or("");
                let Some(world) = self.state.world_or_default(name) else {
                    sender.send_message("§cWorld not found.");
                    return false;
                };
                sender.send_message(&format!(
                    "§7World §f{}§7: generator §f{}§7, seed §f{}§7, time §f{}§7, chunks §f{}§7, entities §f{}",
                    world.name,
                    world.generator.name(),
                    world.seed,
                    world.time(),
                    world.loaded_chunk_count(),
                    world.loaded_entity_count()
                ));
                true
            }
            "time" => {
                if let Some(world) = self.state.default_world() {
                    sender.send_message(&format!("§7Time: §f{}", world.time()));
                    true
                } else {
                    false
                }
            }
            "settime" => {
                let Some(time) = args.get(1).and_then(|t| t.parse::<i64>().ok()) else {
                    sender.send_message("§cUsage: /world settime <0-23999>");
                    return false;
                };
                if let Some(world) = self.state.default_world() {
                    world.set_time(time);
                    sender.send_message(&format!("§7Time set to §f{}", world.time()));
                    true
                } else {
                    false
                }
            }
            "getblock" => {
                let coords: Vec<i32> = args[1..].iter().filter_map(|a| a.parse().ok()).collect();
                if coords.len() != 3 {
                    sender.send_message("§cUsage: /world getblock <x> <y> <z>");
                    return false;
                }
                let Some(world) = self.state.default_world() else {
                    return false;
                };
                let block = world.get_block_at(coords[0], coords[1], coords[2]);
                sender.send_message(&format!(
                    "§7Block at ({}, {}, {}): §f{} §7(id {}, meta {}, hardness {:.2})",
                    coords[0],
                    coords[1],
                    coords[2],
                    block.name(),
                    block.id,
                    block.meta,
                    block.hardness()
                ));
                true
            }
            "setblock" => {
                if args.len() < 5 {
                    sender.send_message("§cUsage: /world setblock <x> <y> <z> <block> [meta]");
                    return false;
                }
                let Ok(x) = args[1].parse::<i32>() else {
                    return false;
                };
                let Ok(y) = args[2].parse::<i32>() else {
                    return false;
                };
                let Ok(z) = args[3].parse::<i32>() else {
                    return false;
                };
                let name = args[4];
                let Some(block) = Block::from_name(name) else {
                    sender.send_message(&format!("§cUnknown block '{}'", name));
                    return false;
                };
                let meta = args.get(5).and_then(|m| m.parse::<u16>().ok()).unwrap_or(0);
                if let Some(world) = self.state.default_world() {
                    world.set_block_at(x, y, z, Block { meta, ..block });
                    sender.send_message(&format!(
                        "§7Placed §f{}§7 at ({}, {}, {})",
                        block.name(),
                        x,
                        y,
                        z
                    ));
                    true
                } else {
                    false
                }
            }
            "tp" => {
                if args.len() < 5 {
                    sender.send_message("§cUsage: /world tp <player> <x> <y> <z>");
                    return false;
                }
                let Some(player) = self.state.players.get(args[1]) else {
                    sender.send_message("§cPlayer not found.");
                    return false;
                };
                let (Ok(x), Ok(y), Ok(z)) = (
                    args[2].parse::<f64>(),
                    args[3].parse::<f64>(),
                    args[4].parse::<f64>(),
                ) else {
                    return false;
                };
                player.set_position(pm_math::Vector3::new(x, y, z));
                sender.send_message(&format!(
                    "§7Teleported §f{}§7 to ({}, {}, {})",
                    player.name, x, y, z
                ));
                true
            }
            _ => {
                sender.send_message(self.usage());
                false
            }
        }
    }
}

/// Simulates server events so the whole Rust <-> C# event pipeline can be tested
/// without a Bedrock client.
pub struct EventCommand {
    state: Arc<ServerState>,
}

impl EventCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for EventCommand {
    fn name(&self) -> &str {
        "event"
    }
    fn description(&self) -> &str {
        "Dispatches a test event to plugins"
    }
    fn usage(&self) -> &str {
        "/event <join|quit|chat|break|place|damage|death|respawn> [player] [text...]"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let Some(kind) = args.first().map(|s| s.to_ascii_lowercase()) else {
            sender.send_message(self.usage());
            return false;
        };
        let player_name = args.get(1).copied().unwrap_or("Steve");
        let text_args = if args.len() > 2 {
            args[2..].join(" ")
        } else {
            String::new()
        };

        match kind.as_str() {
            "join" => {
                let player = self.state.spawn_test_player(player_name);
                let mut event =
                    PlayerJoinEvent::new(player.name.clone(), player.uuid.clone(), String::new());
                self.state.dispatch(&mut event);
                let message = if event.join_message.is_empty() {
                    format!("§e{} joined the game", player.name)
                } else {
                    event.join_message.clone()
                };
                self.state.broadcast(&message);
                sender.send_message("§7Dispatched PlayerJoinEvent");
                true
            }
            "quit" => {
                let player = self.state.spawn_test_player(player_name);
                let mut event =
                    PlayerQuitEvent::new(player.name.clone(), player.uuid.clone(), String::new());
                self.state.dispatch(&mut event);
                let message = if event.quit_message.is_empty() {
                    format!("§e{} left the game", player.name)
                } else {
                    event.quit_message.clone()
                };
                self.state.broadcast(&message);
                self.state.players.remove(&player.name);
                sender.send_message("§7Dispatched PlayerQuitEvent");
                true
            }
            "chat" => {
                let player = self.state.spawn_test_player(player_name);
                let message = if text_args.is_empty() {
                    "hello from the console".to_string()
                } else {
                    text_args
                };
                let mut event = PlayerChatEvent::new(
                    player.name.clone(),
                    player.uuid.clone(),
                    message.clone(),
                    "<{0}> {1}".to_string(),
                );
                let cancelled = self.state.dispatch(&mut event);
                if cancelled {
                    sender.send_message("§7PlayerChatEvent was cancelled by a plugin");
                } else {
                    let format = if event.format.is_empty() {
                        "<{0}> {1}".to_string()
                    } else {
                        event.format.clone()
                    };
                    let line = format
                        .replace("{0}", &player.name)
                        .replace("{1}", &event.message);
                    self.state.broadcast(&format!("§f{}", line));
                }
                true
            }
            "break" => {
                let player = self.state.spawn_test_player(player_name);
                let pos = player.position().floor();
                let mut event = BlockBreakEvent::new(
                    player.name.clone(),
                    pos,
                    0,
                    "minecraft:grass_block".to_string(),
                );
                let cancelled = self.state.dispatch(&mut event);
                if !cancelled {
                    if let Some(world) = self.state.default_world() {
                        world.set_block(&pos, Block::air());
                    }
                }
                sender.send_message(&format!(
                    "§7BlockBreakEvent at {} handled{}",
                    pos,
                    if cancelled { " (cancelled)" } else { "" }
                ));
                true
            }
            "place" => {
                let player = self.state.spawn_test_player(player_name);
                let pos = BlockPosition::new(0, 5, 0);
                let mut event = BlockPlaceEvent::new(
                    player.name.clone(),
                    pos,
                    0,
                    "minecraft:stone".to_string(),
                );
                let cancelled = self.state.dispatch(&mut event);
                if !cancelled {
                    if let Some(world) = self.state.default_world() {
                        world.set_block(&pos, pm_world::blocks::stone());
                    }
                }
                sender.send_message(&format!("§7BlockPlaceEvent at {} handled", pos));
                true
            }
            "damage" => {
                let player = self.state.spawn_test_player(player_name);
                let amount = text_args.parse::<f64>().unwrap_or(2.0);
                let mut event =
                    EntityDamageEvent::new(player.name.clone(), amount, "generic".to_string());
                let cancelled = self.state.dispatch(&mut event);
                if !cancelled {
                    player.set_health(player.health() - event.damage as f32);
                    self.state.broadcast(&format!(
                        "§c{} took {:.1} damage (health {:.1})",
                        player.name,
                        event.damage,
                        player.health()
                    ));
                }
                sender.send_message("§7Dispatched EntityDamageEvent");
                true
            }
            "death" => {
                let player = self.state.spawn_test_player(player_name);
                let mut event = PlayerDeathEvent::new(player.name.clone(), String::new());
                let cancelled = self.state.dispatch(&mut event);
                let message = if event.death_message.is_empty() {
                    format!("§c{} died", player.name)
                } else {
                    event.death_message.clone()
                };
                self.state.broadcast(&message);
                sender.send_message(&format!(
                    "§7PlayerDeathEvent dispatched{}",
                    if cancelled { " (cancelled)" } else { "" }
                ));
                true
            }
            "respawn" => {
                let player = self.state.spawn_test_player(player_name);
                let spawn = self
                    .state
                    .default_world()
                    .map(|w| w.spawn_position())
                    .unwrap_or(pm_math::Vector3::ZERO);
                let mut event =
                    PlayerRespawnEvent::new(player.name.clone(), spawn.x, spawn.y, spawn.z);
                self.state.dispatch(&mut event);
                player.set_position(pm_math::Vector3::new(event.x, event.y, event.z));
                sender.send_message("§7Dispatched PlayerRespawnEvent");
                true
            }
            _ => {
                sender.send_message(self.usage());
                false
            }
        }
    }
}

pub struct KickCommand {
    state: Arc<ServerState>,
}

impl KickCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for KickCommand {
    fn name(&self) -> &str {
        "kick"
    }
    fn description(&self) -> &str {
        "Kicks a player"
    }
    fn usage(&self) -> &str {
        "/kick <player> [reason]"
    }
    fn permission(&self) -> Option<&str> {
        Some("pocketmine.command.kick")
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let Some(name) = args.first() else {
            sender.send_message(self.usage());
            return false;
        };
        let reason = if args.len() > 1 {
            args[1..].join(" ")
        } else {
            "Kicked by an operator".to_string()
        };
        let Some(player) = self.state.players.get(name) else {
            sender.send_message("§cPlayer not found.");
            return false;
        };
        player.send_message(&format!("§cKicked: {}", reason));
        player.set_online(false);
        sender.send_message(&format!("§7Kicked §f{}§7: {}", player.name, reason));
        true
    }
}

pub struct OpCommand {
    state: Arc<ServerState>,
    op: bool,
}

impl OpCommand {
    pub fn new(state: Arc<ServerState>, op: bool) -> Self {
        Self { state, op }
    }
}

impl Command for OpCommand {
    fn name(&self) -> &str {
        if self.op {
            "op"
        } else {
            "deop"
        }
    }
    fn description(&self) -> &str {
        if self.op {
            "Gives operator status"
        } else {
            "Removes operator status"
        }
    }
    fn usage(&self) -> &str {
        if self.op {
            "/op <player>"
        } else {
            "/deop <player>"
        }
    }
    fn permission(&self) -> Option<&str> {
        Some("pocketmine.command.op")
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let Some(name) = args.first() else {
            return false;
        };
        let Some(player) = self.state.players.get(name) else {
            sender.send_message("§cPlayer not found.");
            return false;
        };
        if self.op {
            player.set_op(true);
            sender.send_message(&format!("§7Opped §f{}", player.name));
        } else {
            player.set_op(false);
            sender.send_message(&format!("§7De-opped §f{}", player.name));
        }
        true
    }
}

pub struct SaveCommand {
    state: Arc<ServerState>,
}

impl SaveCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for SaveCommand {
    fn name(&self) -> &str {
        "save"
    }
    fn description(&self) -> &str {
        "Saves all loaded worlds"
    }
    fn usage(&self) -> &str {
        "/save"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        self.state.save_worlds();
        sender.send_message("§aWorlds saved.");
        true
    }
}

pub struct ReloadCommand {
    state: Arc<ServerState>,
}

impl ReloadCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for ReloadCommand {
    fn name(&self) -> &str {
        "reload"
    }
    fn description(&self) -> &str {
        "Reloads plugins"
    }
    fn usage(&self) -> &str {
        "/reload"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        let Some(host) = self.state.plugin_host.get() else {
            sender.send_message("§cC# plugin host is not running.");
            return false;
        };
        host.disable_plugins();
        let count = host.load_plugins("plugins");
        host.enable_plugins();
        sender.send_message(&format!("§aReloaded plugins ({} loaded).", count));
        true
    }
}

pub struct InfoCommand {
    state: Arc<ServerState>,
}

impl InfoCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for InfoCommand {
    fn name(&self) -> &str {
        "info"
    }
    fn description(&self) -> &str {
        "Shows the server summary"
    }
    fn usage(&self) -> &str {
        "/info"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        let info = self.state.server_info();
        sender.send_message(&format!(
            "§b{} §f{}§7 | MC §f{}§7 | API §f{}",
            info["name"].as_str().unwrap_or("?"),
            info["version"].as_str().unwrap_or("?"),
            info["minecraft_version"].as_str().unwrap_or("?"),
            info["api_version"].as_str().unwrap_or("?")
        ));
        sender.send_message(&format!(
            "§7Players: §f{}§7/§f{}§7 | TPS: §f{:.2}§7 | Worlds: §f{}§7 | Plugins: §f{}",
            info["online"],
            info["max_players"],
            info["tps"].as_f64().unwrap_or(0.0),
            info["worlds"].as_array().map(|a| a.len()).unwrap_or(0),
            info["plugins"].as_array().map(|a| a.len()).unwrap_or(0),
        ));
        true
    }
}

/// Registers every built-in command on the server's command map.
pub fn register_default_commands(state: &Arc<ServerState>) {
    let commands: Vec<Arc<dyn Command>> = vec![
        Arc::new(HelpCommand::new(state.clone())),
        Arc::new(StopCommand::new(state.clone())),
        Arc::new(VersionCommand::new(state.clone())),
        Arc::new(SayCommand::new(state.clone())),
        Arc::new(ListCommand::new(state.clone())),
        Arc::new(PluginsCommand::new(state.clone())),
        Arc::new(TpsCommand::new(state.clone())),
        Arc::new(StatusCommand::new(state.clone())),
        Arc::new(WorldCommand::new(state.clone())),
        Arc::new(EventCommand::new(state.clone())),
        Arc::new(KickCommand::new(state.clone())),
        Arc::new(OpCommand::new(state.clone(), true)),
        Arc::new(OpCommand::new(state.clone(), false)),
        Arc::new(ReloadCommand::new(state.clone())),
        Arc::new(InfoCommand::new(state.clone())),
        Arc::new(GamemodeCommand::new(state.clone())),
        Arc::new(GiveCommand::new(state.clone())),
        Arc::new(TeleportCommand::new(state.clone())),
        Arc::new(KillCommand::new(state.clone())),
        Arc::new(SaveCommand::new(state.clone())),
    ];
    for command in commands {
        state.commands.register(command);
    }
}

/// Used by `/stop` to break out of the tick loop.
pub fn request_stop(state: &ServerState) {
    state.running.store(false, Ordering::Relaxed);
}

/// Small helper used in tests.
#[allow(dead_code)]
pub fn dispatch_start(state: &ServerState) {
    state.dispatch(&mut ServerStartEvent::new());
    state.dispatch(&mut ServerStopEvent::new());
    let _ = text::strip_colors("");
}
