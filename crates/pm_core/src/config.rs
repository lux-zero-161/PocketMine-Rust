use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProperties {
    pub motd: String,
    pub server_port: u16,
    pub server_portv6: u16,
    pub gamemode: String,
    pub max_players: u32,
    pub spawn_protection: u32,
    pub white_list: bool,
    pub pvp: bool,
    pub difficulty: u32,
    pub level_name: String,
    pub level_seed: String,
    pub level_type: String,
    pub enable_query: bool,
    pub auto_save: bool,
    pub view_distance: u32,
    pub xbox_auth: bool,
}

impl Default for ServerProperties {
    fn default() -> Self {
        Self {
            motd: "PocketMine-Rust Dedicated Server".to_string(),
            server_port: crate::DEFAULT_PORT,
            server_portv6: crate::DEFAULT_PORT_V6,
            gamemode: "survival".to_string(),
            max_players: 20,
            spawn_protection: 16,
            white_list: false,
            pvp: true,
            difficulty: 1,
            level_name: "world".to_string(),
            level_seed: "".to_string(),
            level_type: "flat".to_string(),
            enable_query: true,
            auto_save: true,
            view_distance: 8,
            xbox_auth: false,
        }
    }
}

impl ServerProperties {
    pub fn load_or_create(path: &Path) -> Self {
        if !path.exists() {
            let default = Self::default();
            default.save(path);
            return default;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };

        let mut map = HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                map.insert(k.trim().to_string(), v.trim().to_string());
            }
        }

        let mut props = Self::default();
        if let Some(v) = map.get("motd") { props.motd = v.clone(); }
        if let Some(v) = map.get("server-port").and_then(|v| v.parse().ok()) { props.server_port = v; }
        if let Some(v) = map.get("server-portv6").and_then(|v| v.parse().ok()) { props.server_portv6 = v; }
        if let Some(v) = map.get("gamemode") { props.gamemode = v.clone(); }
        if let Some(v) = map.get("max-players").and_then(|v| v.parse().ok()) { props.max_players = v; }
        if let Some(v) = map.get("white-list") { props.white_list = v == "on" || v == "true"; }
        if let Some(v) = map.get("pvp") { props.pvp = v == "on" || v == "true"; }
        if let Some(v) = map.get("level-name") { props.level_name = v.clone(); }
        if let Some(v) = map.get("level-type") { props.level_type = v.clone(); }
        if let Some(v) = map.get("view-distance").and_then(|v| v.parse().ok()) { props.view_distance = v; }
        if let Some(v) = map.get("xbox-auth") { props.xbox_auth = v == "on" || v == "true"; }

        props
    }

    pub fn save(&self, path: &Path) {
        let content = format!(
            "# PocketMine-Rust Server Properties\n\
            motd={}\n\
            server-port={}\n\
            server-portv6={}\n\
            gamemode={}\n\
            max-players={}\n\
            spawn-protection={}\n\
            white-list={}\n\
            pvp={}\n\
            difficulty={}\n\
            level-name={}\n\
            level-seed={}\n\
            level-type={}\n\
            enable-query={}\n\
            auto-save={}\n\
            view-distance={}\n\
            xbox-auth={}\n",
            self.motd,
            self.server_port,
            self.server_portv6,
            self.gamemode,
            self.max_players,
            self.spawn_protection,
            if self.white_list { "on" } else { "off" },
            if self.pvp { "on" } else { "off" },
            self.difficulty,
            self.level_name,
            self.level_seed,
            self.level_type,
            if self.enable_query { "on" } else { "off" },
            if self.auto_save { "on" } else { "off" },
            self.view_distance,
            if self.xbox_auth { "on" } else { "off" }
        );
        let _ = fs::write(path, content);
    }
}
