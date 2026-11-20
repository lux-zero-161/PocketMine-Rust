pub mod inventory;

pub use inventory::*;

use pm_command::CommandSender;
use pm_math::Vector3;
use std::collections::HashSet;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Survival = 0,
    Creative = 1,
    Adventure = 2,
    Spectator = 3,
}

pub struct Player {
    pub name: String,
    pub uuid: String,
    pub position: RwLock<Vector3>,
    pub yaw: RwLock<f32>,
    pub pitch: RwLock<f32>,
    pub gamemode: RwLock<GameMode>,
    pub health: RwLock<f32>,
    pub max_health: f32,
    pub inventory: RwLock<PlayerInventory>,
    permissions: RwLock<HashSet<String>>,
    is_op: RwLock<bool>,
    logger: pm_core::Logger,
}

impl Player {
    pub fn new(name: impl Into<String>, uuid: impl Into<String>, spawn_pos: Vector3) -> Self {
        let name_str = name.into();
        Self {
            logger: pm_core::Logger::new(format!("Player: {}", &name_str)),
            name: name_str,
            uuid: uuid.into(),
            position: RwLock::new(spawn_pos),
            yaw: RwLock::new(0.0),
            pitch: RwLock::new(0.0),
            gamemode: RwLock::new(GameMode::Survival),
            health: RwLock::new(20.0),
            max_health: 20.0,
            inventory: RwLock::new(PlayerInventory::new()),
            permissions: RwLock::new(HashSet::new()),
            is_op: RwLock::new(false),
        }
    }

    pub fn teleport(&self, pos: Vector3) {
        let mut p = self.position.write().unwrap();
        *p = pos;
    }

    pub fn set_gamemode(&self, mode: GameMode) {
        let mut gm = self.gamemode.write().unwrap();
        *gm = mode;
    }

    pub fn set_op(&self, op: bool) {
        let mut o = self.is_op.write().unwrap();
        *o = op;
    }

    pub fn add_permission(&self, perm: &str) {
        let mut p = self.permissions.write().unwrap();
        p.insert(perm.to_string());
    }

    pub fn kick(&self, reason: &str) {
        self.logger.info(&format!("Kicked: {}", reason));
    }
}

impl CommandSender for Player {
    fn name(&self) -> &str {
        &self.name
    }

    fn send_message(&self, message: &str) {
        self.logger.info(&format!("[To {}]: {}", self.name, message));
    }

    fn has_permission(&self, permission: &str) -> bool {
        if *self.is_op.read().unwrap() {
            return true;
        }
        let perms = self.permissions.read().unwrap();
        perms.contains(permission) || perms.contains("*")
    }

    fn is_player(&self) -> bool {
        true
    }
}
