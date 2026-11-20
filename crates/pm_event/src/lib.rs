use pm_math::BlockPosition;
use serde::{Deserialize, Serialize};

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

pub trait Cancellable {
    fn is_cancelled(&self) -> bool;
    fn set_cancelled(&mut self, cancel: bool);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum EventTypeId {
    PlayerJoin = 1,
    PlayerQuit = 2,
    PlayerChat = 3,
    BlockBreak = 4,
    BlockPlace = 5,
    ServerCommand = 6,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerJoinEvent {
    pub player_name: String,
    pub join_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerQuitEvent {
    pub player_name: String,
    pub quit_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerChatEvent {
    pub player_name: String,
    pub message: String,
    pub format: String,
    pub is_cancelled: bool,
}

impl Cancellable for PlayerChatEvent {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled
    }
    fn set_cancelled(&mut self, cancel: bool) {
        self.is_cancelled = cancel;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockBreakEvent {
    pub player_name: String,
    pub block_pos: BlockPosition,
    pub block_id: u32,
    pub is_cancelled: bool,
}

impl Cancellable for BlockBreakEvent {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled
    }
    fn set_cancelled(&mut self, cancel: bool) {
        self.is_cancelled = cancel;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockPlaceEvent {
    pub player_name: String,
    pub block_pos: BlockPosition,
    pub block_id: u32,
    pub is_cancelled: bool,
}

impl Cancellable for BlockPlaceEvent {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled
    }
    fn set_cancelled(&mut self, cancel: bool) {
        self.is_cancelled = cancel;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerCommandEvent {
    pub sender_name: String,
    pub command: String,
    pub is_cancelled: bool,
}

impl Cancellable for ServerCommandEvent {
    fn is_cancelled(&self) -> bool {
        self.is_cancelled
    }
    fn set_cancelled(&mut self, cancel: bool) {
        self.is_cancelled = cancel;
    }
}
