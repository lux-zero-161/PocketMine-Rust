use crate::server::ServerState;
use pm_command::{Command, CommandSender};
use pm_player::{GameMode, ItemStack};
use std::sync::Arc;

pub struct GamemodeCommand {
    state: Arc<ServerState>,
}

impl GamemodeCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for GamemodeCommand {
    fn name(&self) -> &str {
        "gamemode"
    }
    fn description(&self) -> &str {
        "Changes a player's gamemode"
    }
    fn usage(&self) -> &str {
        "/gamemode <survival|creative|adventure|spectator> [player]"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn permission(&self) -> Option<&str> {
        Some("pocketmine.command.gamemode")
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let Some(mode) = args.first() else {
            sender.send_message(self.usage());
            return false;
        };
        let mode = GameMode::from_name(mode);
        let target_name = args.get(1).copied().unwrap_or(sender.name());
        let Some(player) = self.state.players.get(target_name) else {
            sender.send_message(&format!("§cPlayer '{}' not found.", target_name));
            return false;
        };
        player.set_gamemode(mode);
        sender.send_message(&format!(
            "§7Set gamemode of §f{}§7 to §f{}",
            player.name,
            mode.name()
        ));
        player.send_message(&format!("§7Your gamemode is now §f{}", mode.name()));
        true
    }
}

pub struct GiveCommand {
    state: Arc<ServerState>,
}

impl GiveCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for GiveCommand {
    fn name(&self) -> &str {
        "give"
    }
    fn description(&self) -> &str {
        "Gives an item to a player"
    }
    fn usage(&self) -> &str {
        "/give <player> <item> [count]"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn permission(&self) -> Option<&str> {
        Some("pocketmine.command.give")
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        if args.len() < 2 {
            sender.send_message(self.usage());
            return false;
        }
        let Some(player) = self.state.players.get(args[0]) else {
            sender.send_message(&format!("§cPlayer '{}' not found.", args[0]));
            return false;
        };
        let item_name = args[1];
        if pm_data::item_by_name(item_name).is_none()
            && pm_data::block_properties(item_name).is_none()
        {
            sender.send_message(&format!("§cUnknown item '{}'.", item_name));
            return false;
        }
        let count: u16 = args
            .get(2)
            .and_then(|c| c.parse().ok())
            .unwrap_or(1)
            .clamp(1, 64);
        let item = ItemStack::new(item_name, count);
        let leftover = {
            let mut inventory = player.inventory_mut();
            inventory.add_item(item.clone())
        };
        if leftover > 0 {
            sender.send_message(&format!(
                "§eInventory full, {} item(s) were not added.",
                leftover
            ));
        }
        sender.send_message(&format!(
            "§7Gave §f{}§7 x§f{}§7 to §f{}",
            item_name,
            count - leftover,
            player.name
        ));
        player.send_message(&format!(
            "§7You received §f{}§7 x§f{}",
            item_name,
            count - leftover
        ));
        true
    }
}

pub struct TeleportCommand {
    state: Arc<ServerState>,
}

impl TeleportCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for TeleportCommand {
    fn name(&self) -> &str {
        "tp"
    }
    fn description(&self) -> &str {
        "Teleports a player"
    }
    fn usage(&self) -> &str {
        "/tp <player> <x> <y> <z>"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn permission(&self) -> Option<&str> {
        Some("pocketmine.command.teleport")
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        if args.len() != 4 {
            sender.send_message(self.usage());
            return false;
        }
        let Some(player) = self.state.players.get(args[0]) else {
            sender.send_message(&format!("§cPlayer '{}' not found.", args[0]));
            return false;
        };
        let (Ok(x), Ok(y), Ok(z)) = (
            args[1].parse::<f64>(),
            args[2].parse::<f64>(),
            args[3].parse::<f64>(),
        ) else {
            sender.send_message("§cCoordinates must be numbers.");
            return false;
        };
        player.set_position(pm_math::Vector3::new(x, y, z));
        sender.send_message(&format!(
            "§7Teleported §f{}§7 to ({}, {}, {})",
            player.name, x, y, z
        ));
        player.send_message(&format!("§7You were teleported to ({}, {}, {})", x, y, z));
        true
    }
}

pub struct KillCommand {
    state: Arc<ServerState>,
}

impl KillCommand {
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }
}

impl Command for KillCommand {
    fn name(&self) -> &str {
        "kill"
    }
    fn description(&self) -> &str {
        "Kills a player"
    }
    fn usage(&self) -> &str {
        "/kill <player>"
    }
    fn aliases(&self) -> &[String] {
        const ALIASES: &[String] = &[];
        ALIASES
    }
    fn permission(&self) -> Option<&str> {
        Some("pocketmine.command.kill")
    }
    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        let Some(name) = args.first() else {
            sender.send_message(self.usage());
            return false;
        };
        let Some(player) = self.state.players.get(name) else {
            sender.send_message(&format!("§cPlayer '{}' not found.", name));
            return false;
        };
        let mut event = pm_event::PlayerDeathEvent::new(player.name.clone(), String::new());
        let cancelled = self.state.dispatch(&mut event);
        if cancelled {
            sender.send_message("§7Kill was cancelled by a plugin.");
            return false;
        }
        player.set_health(0.0);
        player.send_message("§cYou died.");
        let message = if event.death_message.is_empty() {
            format!("§c{} died", player.name)
        } else {
            event.death_message.clone()
        };
        self.state.broadcast(&message);
        true
    }
}
