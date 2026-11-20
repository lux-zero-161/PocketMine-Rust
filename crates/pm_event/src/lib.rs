use pm_math::BlockPosition;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum EventPriority {
    Lowest = 0,
    Low = 1,
    Normal = 2,
    High = 3,
    Highest = 4,
    Monitor = 5,
}

impl EventPriority {
    pub fn as_int(&self) -> i32 {
        *self as i32
    }
}

pub trait Cancellable {
    fn is_cancelled(&self) -> bool;
    fn set_cancelled(&mut self, cancel: bool);
}

/// Every event that can be sent to C# plugins has a stable string name. The
/// names match PocketMine-MP's class names in lower camel case so PHP plugin
/// logic ports over mechanically.
pub trait Event: Send {
    fn name(&self) -> &'static str;
    fn is_cancellable(&self) -> bool {
        false
    }
    fn is_cancelled(&self) -> bool {
        false
    }
    fn set_cancelled(&mut self, _cancel: bool) {}
    fn to_json(&self) -> Value;

    /// Applies a patch produced by a plugin (C#) to this event. Only the keys
    /// present in `patch` change, which lets plugins mutate messages, damage,
    /// target positions and similar fields without echoing the whole payload.
    fn apply_patch(&mut self, patch: &Value) {
        let _ = patch;
    }
}

/// Recursively merges `patch` into `base` (objects only; other values replace).
pub fn merge_json(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base_map), Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                merge_json(base_map.entry(key.clone()).or_insert(Value::Null), value);
            }
        }
        (base_slot, value) => {
            *base_slot = value.clone();
        }
    }
}

macro_rules! cancellable_event {
    ($name:ident, $event_name:literal, { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name {
            $(pub $field: $ty,)*
            #[serde(default)]
            pub is_cancelled: bool,
        }

        impl $name {
            pub fn new($($field: $ty),*) -> Self {
                Self { $($field,)* is_cancelled: false }
            }
        }

        impl Cancellable for $name {
            fn is_cancelled(&self) -> bool { self.is_cancelled }
            fn set_cancelled(&mut self, cancel: bool) { self.is_cancelled = cancel; }
        }

        impl Event for $name {
            fn name(&self) -> &'static str { $event_name }
            fn is_cancellable(&self) -> bool { true }
            fn is_cancelled(&self) -> bool { self.is_cancelled }
            fn set_cancelled(&mut self, cancel: bool) { self.is_cancelled = cancel; }
            fn to_json(&self) -> Value { serde_json::to_value(self).unwrap_or(Value::Null) }
            fn apply_patch(&mut self, patch: &Value) {
                let mut base = serde_json::to_value(&*self).unwrap_or(Value::Null);
                merge_json(&mut base, patch);
                if let Ok(updated) = serde_json::from_value::<Self>(base) {
                    *self = updated;
                }
            }
        }

        impl $name {
            /// Serialises the event and appends the cancellation flag.
            pub fn payload(&self) -> String { self.to_json().to_string() }
        }
    };
}

macro_rules! plain_event {
    ($name:ident, $event_name:literal, { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct $name {
            $(pub $field: $ty,)*
        }

        impl $name {
            pub fn new($($field: $ty),*) -> Self { Self { $($field,)* } }
        }

        impl Event for $name {
            fn name(&self) -> &'static str { $event_name }
            fn to_json(&self) -> Value { serde_json::to_value(self).unwrap_or(Value::Null) }
            fn apply_patch(&mut self, patch: &Value) {
                let mut base = serde_json::to_value(&*self).unwrap_or(Value::Null);
                merge_json(&mut base, patch);
                if let Ok(updated) = serde_json::from_value::<Self>(base) {
                    *self = updated;
                }
            }
        }

        impl $name {
            pub fn payload(&self) -> String { self.to_json().to_string() }
        }
    };
}

plain_event!(PlayerJoinEvent, "player.join", {
    player_name: String,
    player_uuid: String,
    join_message: String,
});

plain_event!(PlayerQuitEvent, "player.quit", {
    player_name: String,
    player_uuid: String,
    quit_message: String,
});

cancellable_event!(PlayerChatEvent, "player.chat", {
    player_name: String,
    player_uuid: String,
    message: String,
    format: String,
});

cancellable_event!(PlayerCommandPreprocessEvent, "player.command_preprocess", {
    player_name: String,
    message: String,
});

cancellable_event!(PlayerMoveEvent, "player.move", {
    player_name: String,
    from_x: f64,
    from_y: f64,
    from_z: f64,
    to_x: f64,
    to_y: f64,
    to_z: f64,
});

cancellable_event!(PlayerInteractEvent, "player.interact", {
    player_name: String,
    block_x: i32,
    block_y: i32,
    block_z: i32,
    block_name: String,
    action: String,
});

cancellable_event!(BlockBreakEvent, "block.break", {
    player_name: String,
    block_pos: BlockPosition,
    block_id: u32,
    block_name: String,
});

cancellable_event!(BlockPlaceEvent, "block.place", {
    player_name: String,
    block_pos: BlockPosition,
    block_id: u32,
    block_name: String,
});

cancellable_event!(EntityDamageEvent, "entity.damage", {
    entity_name: String,
    damage: f64,
    cause: String,
});

cancellable_event!(PlayerDeathEvent, "player.death", {
    player_name: String,
    death_message: String,
});

plain_event!(PlayerRespawnEvent, "player.respawn", {
    player_name: String,
    x: f64,
    y: f64,
    z: f64,
});

cancellable_event!(ServerCommandEvent, "server.command", {
    sender_name: String,
    command: String,
});

plain_event!(ServerStartEvent, "server.start", {});
plain_event!(ServerStopEvent, "server.stop", {});
plain_event!(WorldLoadEvent, "world.load", { world_name: String });
plain_event!(WorldUnloadEvent, "world.unload", { world_name: String });

/// Legacy numeric IDs kept for the plugin ABI used by the existing C# host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum EventTypeId {
    PlayerJoin = 1,
    PlayerQuit = 2,
    PlayerChat = 3,
    BlockBreak = 4,
    BlockPlace = 5,
    ServerCommand = 6,
    PlayerCommandPreprocess = 7,
    PlayerMove = 8,
    PlayerInteract = 9,
    EntityDamage = 10,
    PlayerDeath = 11,
    PlayerRespawn = 12,
    ServerStart = 13,
    ServerStop = 14,
    WorldLoad = 15,
    WorldUnload = 16,
}

impl EventTypeId {
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "player.join" => EventTypeId::PlayerJoin,
            "player.quit" => EventTypeId::PlayerQuit,
            "player.chat" => EventTypeId::PlayerChat,
            "block.break" => EventTypeId::BlockBreak,
            "block.place" => EventTypeId::BlockPlace,
            "server.command" => EventTypeId::ServerCommand,
            "player.command_preprocess" => EventTypeId::PlayerCommandPreprocess,
            "player.move" => EventTypeId::PlayerMove,
            "player.interact" => EventTypeId::PlayerInteract,
            "entity.damage" => EventTypeId::EntityDamage,
            "player.death" => EventTypeId::PlayerDeath,
            "player.respawn" => EventTypeId::PlayerRespawn,
            "server.start" => EventTypeId::ServerStart,
            "server.stop" => EventTypeId::ServerStop,
            "world.load" => EventTypeId::WorldLoad,
            "world.unload" => EventTypeId::WorldUnload,
            _ => return None,
        })
    }
}
