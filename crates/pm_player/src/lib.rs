//! Player model, inventory and session registry.

use pm_command::CommandSender;
use pm_data;
use pm_math::{Location, Vector3};
use pm_permission::PermissionAttachment;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

pub const HOTBAR_SIZE: usize = 9;
pub const INVENTORY_SIZE: usize = 27;
pub const ARMOR_SIZE: usize = 4;
pub const TOTAL_INVENTORY_SIZE: usize = HOTBAR_SIZE + INVENTORY_SIZE + ARMOR_SIZE + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 6,
}

impl GameMode {
    pub fn from_id(id: i32) -> Self {
        match id {
            1 => GameMode::Creative,
            2 => GameMode::Adventure,
            6 => GameMode::Spectator,
            _ => GameMode::Survival,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "creative" | "c" | "1" => GameMode::Creative,
            "adventure" | "a" | "2" => GameMode::Adventure,
            "spectator" | "sp" | "6" => GameMode::Spectator,
            "survival" | "s" | "0" => GameMode::Survival,
            _ => GameMode::Survival,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            GameMode::Survival => "Survival",
            GameMode::Creative => "Creative",
            GameMode::Adventure => "Adventure",
            GameMode::Spectator => "Spectator",
        }
    }

    pub fn allows_flight(&self) -> bool {
        matches!(self, GameMode::Creative | GameMode::Spectator)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: i32,
    pub meta: u16,
    pub count: u16,
    pub name: String,
    pub custom_name: Option<String>,
}

impl ItemStack {
    pub fn null() -> Self {
        Self {
            id: 0,
            meta: 0,
            count: 0,
            name: pm_data::items::AIR.to_string(),
            custom_name: None,
        }
    }

    pub fn new(name: &str, count: u16) -> Self {
        let id = pm_data::item_by_name(name)
            .map(|i| i.numeric_id)
            .unwrap_or(0);
        Self {
            id,
            meta: 0,
            count,
            name: name.to_string(),
            custom_name: None,
        }
    }

    pub fn is_null(&self) -> bool {
        self.count == 0 || self.id == 0
    }

    pub fn set_custom_name(&mut self, name: impl Into<String>) {
        self.custom_name = Some(name.into());
    }

    /// Item identifier, resolved through the bedrock-data registry.
    pub fn type_name(&self) -> &str {
        &self.name
    }

    pub fn max_stack_size(&self) -> u16 {
        // Tools/armour are unstackable; everything else stacks to 64.
        if self.is_null() {
            0
        } else {
            64
        }
    }

    pub fn equals(&self, other: &ItemStack) -> bool {
        self.id == other.id && self.meta == other.meta
    }
}

impl Default for ItemStack {
    fn default() -> Self {
        Self::null()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemSlot {
    pub item: ItemStack,
}

impl ItemSlot {
    pub fn empty() -> Self {
        Self {
            item: ItemStack::null(),
        }
    }
    pub fn new(item: ItemStack) -> Self {
        Self { item }
    }
    pub fn is_empty(&self) -> bool {
        self.item.is_null()
    }
}

#[derive(Debug, Clone)]
pub struct PlayerInventory {
    /// Slots 0..8 are the hotbar, 9..35 the main inventory.
    slots: Vec<ItemSlot>,
    armor: Vec<ItemSlot>,
    offhand: ItemSlot,
    held_slot: usize,
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerInventory {
    pub fn new() -> Self {
        Self {
            slots: vec![ItemSlot::empty(); HOTBAR_SIZE + INVENTORY_SIZE],
            armor: vec![ItemSlot::empty(); ARMOR_SIZE],
            offhand: ItemSlot::empty(),
            held_slot: 0,
        }
    }

    pub fn size(&self) -> usize {
        self.slots.len() + self.armor.len() + 1
    }

    pub fn get(&self, slot: usize) -> Option<&ItemSlot> {
        if slot < self.slots.len() {
            self.slots.get(slot)
        } else if slot < self.slots.len() + self.armor.len() {
            self.armor.get(slot - self.slots.len())
        } else if slot == self.slots.len() + self.armor.len() {
            Some(&self.offhand)
        } else {
            None
        }
    }

    pub fn set(&mut self, slot: usize, item: ItemStack) -> bool {
        if slot < self.slots.len() {
            self.slots[slot] = ItemSlot::new(item);
            true
        } else if slot < self.slots.len() + self.armor.len() {
            self.armor[slot - self.slots.len()] = ItemSlot::new(item);
            true
        } else if slot == self.slots.len() + self.armor.len() {
            self.offhand = ItemSlot::new(item);
            true
        } else {
            false
        }
    }

    pub fn held_item(&self) -> &ItemStack {
        &self.slots[self.held_slot.min(self.slots.len() - 1)].item
    }

    pub fn held_slot(&self) -> usize {
        self.held_slot
    }

    pub fn set_held_slot(&mut self, slot: usize) {
        self.held_slot = slot.min(HOTBAR_SIZE - 1);
    }

    /// Adds an item stack, returning the leftover count that did not fit.
    pub fn add_item(&mut self, mut item: ItemStack) -> u16 {
        if item.is_null() {
            return 0;
        }
        for slot in &mut self.slots {
            if slot.item.equals(&item) && slot.item.count < slot.item.max_stack_size() {
                let space = slot.item.max_stack_size() - slot.item.count;
                let moved = space.min(item.count);
                slot.item.count += moved;
                item.count -= moved;
                if item.count == 0 {
                    return 0;
                }
            }
        }
        for slot in &mut self.slots {
            if slot.is_empty() {
                slot.item = item.clone();
                return 0;
            }
        }
        item.count
    }

    pub fn remove_item(&mut self, item: &ItemStack, mut amount: u16) -> bool {
        for slot in &mut self.slots {
            if slot.item.equals(item) && !slot.is_empty() {
                let taken = amount.min(slot.item.count);
                slot.item.count -= taken;
                amount -= taken;
                if slot.item.count == 0 {
                    slot.item = ItemStack::null();
                }
                if amount == 0 {
                    return true;
                }
            }
        }
        false
    }

    pub fn clear(&mut self) {
        for slot in &mut self.slots {
            *slot = ItemSlot::empty();
        }
        for slot in &mut self.armor {
            *slot = ItemSlot::empty();
        }
        self.offhand = ItemSlot::empty();
    }

    pub fn contents(&self) -> Vec<ItemStack> {
        self.slots.iter().map(|s| s.item.clone()).collect()
    }
}

/// Session state for a connected player. The network layer owns the socket and
/// feeds this structure; plugins see a stable snapshot through the C# SDK.
pub struct Player {
    pub name: String,
    pub display_name: String,
    pub uuid: String,
    pub xuid: String,
    pub entity_id: u64,
    pub session_id: u64,
    pub address: String,
    pub port: u16,
    pub position: RwLock<Vector3>,
    pub yaw: RwLock<f64>,
    pub pitch: RwLock<f64>,
    pub world_name: RwLock<String>,
    pub gamemode: RwLock<GameMode>,
    pub health: RwLock<f32>,
    pub max_health: RwLock<f32>,
    pub food: RwLock<u32>,
    pub xp_level: RwLock<u32>,
    pub inventory: RwLock<PlayerInventory>,
    pub permissions: RwLock<Vec<PermissionAttachment>>,
    pub op: RwLock<bool>,
    pub online: RwLock<bool>,
    pub connected_at: std::time::Instant,
    outbox: Mutex<Vec<String>>,
    pub device_os: RwLock<u32>,
}

impl Player {
    pub fn new(
        name: impl Into<String>,
        uuid: impl Into<String>,
        position: Vector3,
        world_name: impl Into<String>,
    ) -> Self {
        let name = name.into();
        Self {
            display_name: name.clone(),
            name,
            uuid: uuid.into(),
            xuid: String::new(),
            entity_id: 0,
            session_id: 0,
            address: "127.0.0.1".to_string(),
            port: 0,
            position: RwLock::new(position),
            yaw: RwLock::new(0.0),
            pitch: RwLock::new(0.0),
            world_name: RwLock::new(world_name.into()),
            gamemode: RwLock::new(GameMode::Survival),
            health: RwLock::new(20.0),
            max_health: RwLock::new(20.0),
            food: RwLock::new(20),
            xp_level: RwLock::new(0),
            inventory: RwLock::new(PlayerInventory::new()),
            permissions: RwLock::new(Vec::new()),
            op: RwLock::new(false),
            online: RwLock::new(true),
            connected_at: std::time::Instant::now(),
            outbox: Mutex::new(Vec::new()),
            device_os: RwLock::new(0),
        }
    }

    pub fn position(&self) -> Vector3 {
        *self.position.read().unwrap()
    }

    pub fn set_position(&self, position: Vector3) {
        *self.position.write().unwrap() = position;
    }

    pub fn location(&self) -> Location {
        Location::new(
            self.position().x,
            self.position().y,
            self.position().z,
            *self.yaw.read().unwrap(),
            *self.pitch.read().unwrap(),
        )
    }

    pub fn world_name(&self) -> String {
        self.world_name.read().unwrap().clone()
    }

    pub fn gamemode(&self) -> GameMode {
        *self.gamemode.read().unwrap()
    }

    pub fn set_gamemode(&self, mode: GameMode) {
        *self.gamemode.write().unwrap() = mode;
    }

    pub fn health(&self) -> f32 {
        *self.health.read().unwrap()
    }

    pub fn set_health(&self, health: f32) {
        *self.health.write().unwrap() = health.clamp(0.0, *self.max_health.read().unwrap());
    }

    pub fn is_online(&self) -> bool {
        *self.online.read().unwrap()
    }

    pub fn set_online(&self, online: bool) {
        *self.online.write().unwrap() = online;
    }

    pub fn is_op(&self) -> bool {
        *self.op.read().unwrap()
    }

    pub fn set_op(&self, op: bool) {
        *self.op.write().unwrap() = op;
    }

    pub fn inventory(&self) -> std::sync::RwLockReadGuard<'_, PlayerInventory> {
        self.inventory.read().unwrap()
    }

    pub fn inventory_mut(&self) -> std::sync::RwLockWriteGuard<'_, PlayerInventory> {
        self.inventory.write().unwrap()
    }

    pub fn add_permission_attachment(&self, attachment: PermissionAttachment) {
        self.permissions.write().unwrap().push(attachment);
    }

    pub fn remove_permission_attachments(&self, plugin: &str) {
        self.permissions
            .write()
            .unwrap()
            .retain(|a| a.plugin.as_deref() != Some(plugin));
    }

    pub fn has_permission_node(&self, node: &str) -> bool {
        if self.is_op() {
            return true;
        }
        let attachments = self.permissions.read().unwrap();
        if let Some(value) = attachments
            .iter()
            .rev()
            .find_map(|a| a.get_permission(node))
        {
            return value;
        }
        false
    }

    /// Queues a chat/raw message for the network layer.
    pub fn send_message(&self, message: &str) {
        self.outbox.lock().unwrap().push(message.to_string());
    }

    pub fn send_title(&self, title: &str, subtitle: &str, fade_in: u32, stay: u32, fade_out: u32) {
        self.outbox.lock().unwrap().push(format!(
            "\u{0}TITLE\u{0}{title}\u{0}{subtitle}\u{0}{fade_in}\u{0}{stay}\u{0}{fade_out}"
        ));
    }

    pub fn pop_outbox(&self) -> Vec<String> {
        std::mem::take(&mut *self.outbox.lock().unwrap())
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "display_name": self.display_name,
            "uuid": self.uuid,
            "xuid": self.xuid,
            "entity_id": self.entity_id,
            "session_id": self.session_id,
            "address": self.address,
            "port": self.port,
            "x": self.position().x,
            "y": self.position().y,
            "z": self.position().z,
            "yaw": *self.yaw.read().unwrap(),
            "pitch": *self.pitch.read().unwrap(),
            "world": self.world_name(),
            "gamemode": self.gamemode() as i32,
            "gamemode_name": self.gamemode().name(),
            "health": self.health(),
            "max_health": *self.max_health.read().unwrap(),
            "food": *self.food.read().unwrap(),
            "xp_level": *self.xp_level.read().unwrap(),
            "is_op": self.is_op(),
            "online": self.is_online(),
            "held_item": self.inventory().held_item().name.clone(),
            "inventory": self.inventory_json(),
        })
    }

    fn inventory_json(&self) -> serde_json::Value {
        let inventory = self.inventory.read().unwrap();
        let mut slots = Vec::new();
        for slot in 0..inventory.size() {
            let Some(entry) = inventory.get(slot) else { continue };
            if entry.is_empty() {
                continue;
            }
            slots.push(serde_json::json!({
                "slot": slot,
                "name": entry.item.name,
                "id": entry.item.id,
                "meta": entry.item.meta,
                "count": entry.item.count,
            }));
        }
        serde_json::Value::Array(slots)
    }
}

impl CommandSender for Player {
    fn name(&self) -> &str {
        &self.name
    }

    fn send_message(&self, message: &str) {
        Player::send_message(self, message);
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.has_permission_node(permission)
    }

    fn is_player(&self) -> bool {
        true
    }

    fn is_op(&self) -> bool {
        Player::is_op(self)
    }
}

/// Registry of every known player (online and offline), keyed by lowercase name.
pub struct PlayerManager {
    players: RwLock<HashMap<String, Arc<Player>>>,
    by_uuid: RwLock<HashMap<String, String>>,
}

impl PlayerManager {
    pub fn new() -> Self {
        Self {
            players: RwLock::new(HashMap::new()),
            by_uuid: RwLock::new(HashMap::new()),
        }
    }

    pub fn add(&self, player: Arc<Player>) {
        let key = player.name.to_ascii_lowercase();
        self.by_uuid
            .write()
            .unwrap()
            .insert(player.uuid.clone(), key.clone());
        self.players.write().unwrap().insert(key, player);
    }

    pub fn get(&self, name: &str) -> Option<Arc<Player>> {
        self.players
            .read()
            .unwrap()
            .get(&name.to_ascii_lowercase())
            .cloned()
    }

    pub fn get_by_uuid(&self, uuid: &str) -> Option<Arc<Player>> {
        let key = self.by_uuid.read().unwrap().get(uuid).cloned()?;
        self.players.read().unwrap().get(&key).cloned()
    }

    pub fn remove(&self, name: &str) -> Option<Arc<Player>> {
        let key = name.to_ascii_lowercase();
        let player = self.players.write().unwrap().remove(&key);
        if let Some(p) = &player {
            self.by_uuid.write().unwrap().remove(&p.uuid);
        }
        player
    }

    pub fn online(&self) -> Vec<Arc<Player>> {
        let mut players: Vec<Arc<Player>> = self
            .players
            .read()
            .unwrap()
            .values()
            .filter(|p| p.is_online())
            .cloned()
            .collect();
        players.sort_by(|a, b| {
            a.name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase())
        });
        players
    }

    pub fn count(&self) -> usize {
        self.online().len()
    }

    pub fn is_online(&self, name: &str) -> bool {
        self.get(name).is_some_and(|p| p.is_online())
    }

    pub fn broadcast(&self, message: &str) {
        for player in self.online() {
            player.send_message(message);
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.online().into_iter().map(|p| p.name.clone()).collect()
    }
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_stacks_and_removes() {
        let mut inv = PlayerInventory::new();
        assert_eq!(inv.add_item(ItemStack::new("minecraft:diamond", 10)), 0);
        assert_eq!(inv.held_item().count, 10);
        assert_eq!(inv.add_item(ItemStack::new("minecraft:diamond", 60)), 0);
        assert_eq!(inv.held_item().count, 64);
        // 70 diamonds overflow the first stack; the rest continues in slot 1.
        assert_eq!(inv.get(1).unwrap().item.count, 6);
        assert_eq!(inv.add_item(ItemStack::new("minecraft:diamond", 5)), 0);
        assert_eq!(inv.get(1).unwrap().item.count, 11);
        assert!(inv.remove_item(&ItemStack::new("minecraft:diamond", 1), 64));
    }

    #[test]
    fn player_manager_roundtrip() {
        let manager = PlayerManager::new();
        let player = Arc::new(Player::new("Steve", "uuid-1", Vector3::ZERO, "world"));
        manager.add(player.clone());
        assert!(manager.is_online("steve"));
        assert_eq!(manager.count(), 1);
        player.set_online(false);
        assert_eq!(manager.count(), 0);
        assert!(manager.remove("Steve").is_some());
    }
}

#[cfg(test)]
mod json_tests {
    use super::*;

    #[test]
    fn inventory_serialises() {
        let player = Player::new("Json", "uuid", Vector3::ZERO, "world");
        let leftover = player.inventory_mut().add_item(ItemStack::new("minecraft:diamond", 5));
        assert_eq!(leftover, 0);
        let value = player.to_json();
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
        assert_eq!(value["inventory"][0]["count"], 5);
        assert_eq!(value["inventory"][0]["name"], "minecraft:diamond");
    }
}
