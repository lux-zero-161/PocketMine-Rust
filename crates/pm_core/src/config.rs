//! `server.properties` handling.
//!
//! PocketMine-MP 5 uses `server.properties` with the same keys as vanilla
//! Bedrock. Unknown keys are preserved so a file written by vanilla/PMMP keeps
//! working after the server rewrites it.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gamemode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl Gamemode {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "creative" | "1" => Gamemode::Creative,
            "adventure" | "2" => Gamemode::Adventure,
            "spectator" | "6" => Gamemode::Spectator,
            _ => Gamemode::Survival,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Gamemode::Survival => "survival",
            Gamemode::Creative => "creative",
            Gamemode::Adventure => "adventure",
            Gamemode::Spectator => "spectator",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerProperties {
    pub motd: String,
    pub sub_motd: String,
    pub server_ip: String,
    pub server_port: u16,
    pub server_port_v6: u16,
    pub enable_query: bool,
    pub query_port: u16,
    pub gamemode: Gamemode,
    pub force_gamemode: bool,
    pub difficulty: u32,
    pub max_players: u32,
    pub online_mode: bool,
    pub allow_list: bool,
    pub white_list: bool,
    pub spawn_protection: u32,
    pub allow_flight: bool,
    pub enable_command_blocks: bool,
    pub level_name: String,
    pub level_seed: String,
    pub level_type: String,
    pub default_permission: String,
    pub player_idle_timeout: u32,
    pub compression_threshold: i32,
    pub compression_algorithm: String,
    pub view_distance: u32,
    pub tick_distance: i32,
    pub xbox_auth: bool,
    pub auto_save: bool,
    pub extra: BTreeMap<String, String>,
}

impl Default for ServerProperties {
    fn default() -> Self {
        Self {
            motd: "PocketMine-Rust Server".to_string(),
            sub_motd: "§bPowered by Rust + C#".to_string(),
            server_ip: "0.0.0.0".to_string(),
            server_port: crate::DEFAULT_PORT,
            server_port_v6: crate::DEFAULT_PORT_V6,
            enable_query: true,
            query_port: crate::DEFAULT_QUERY_PORT,
            gamemode: Gamemode::Survival,
            force_gamemode: true,
            difficulty: 1,
            max_players: crate::DEFAULT_MAX_PLAYERS,
            online_mode: true,
            allow_list: false,
            white_list: false,
            spawn_protection: 16,
            allow_flight: false,
            enable_command_blocks: true,
            level_name: "world".to_string(),
            level_seed: String::new(),
            level_type: "DEFAULT".to_string(),
            default_permission: "op".to_string(),
            player_idle_timeout: 0,
            compression_threshold: 256,
            compression_algorithm: "zlib".to_string(),
            view_distance: crate::DEFAULT_VIEW_DISTANCE,
            tick_distance: crate::DEFAULT_TICK_DISTANCE,
            xbox_auth: true,
            auto_save: true,
            extra: BTreeMap::new(),
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
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Self {
        let mut props = Self::default();
        let mut known = BTreeMap::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            known.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
        for (key, value) in &known {
            let truthy = value.eq_ignore_ascii_case("on")
                || value.eq_ignore_ascii_case("true")
                || value == "1";
            match key.as_str() {
                "motd" => props.motd = value.clone(),
                "sub-motd" => props.sub_motd = value.clone(),
                "server-ip" => props.server_ip = value.clone(),
                "server-port" => props.server_port = value.parse().unwrap_or(props.server_port),
                "server-portv6" => {
                    props.server_port_v6 = value.parse().unwrap_or(props.server_port_v6)
                }
                "enable-query" => props.enable_query = truthy,
                "query-port" => props.query_port = value.parse().unwrap_or(props.query_port),
                "gamemode" => props.gamemode = Gamemode::parse(value),
                "force-gamemode" => props.force_gamemode = truthy,
                "difficulty" => props.difficulty = value.parse().unwrap_or(props.difficulty),
                "max-players" => props.max_players = value.parse().unwrap_or(props.max_players),
                "online-mode" => props.online_mode = truthy,
                "allow-list" => props.allow_list = truthy,
                "white-list" | "whitelist" => props.white_list = truthy,
                "spawn-protection" => {
                    props.spawn_protection = value.parse().unwrap_or(props.spawn_protection)
                }
                "allow-flight" => props.allow_flight = truthy,
                "enable-command-blocks" => props.enable_command_blocks = truthy,
                "level-name" => props.level_name = value.clone(),
                "level-seed" => props.level_seed = value.clone(),
                "level-type" => props.level_type = value.clone(),
                "default-permission" => props.default_permission = value.clone(),
                "player-idle-timeout" => {
                    props.player_idle_timeout = value.parse().unwrap_or(props.player_idle_timeout)
                }
                "compression-threshold" => {
                    props.compression_threshold =
                        value.parse().unwrap_or(props.compression_threshold)
                }
                "compression-algorithm" => props.compression_algorithm = value.clone(),
                "view-distance" => {
                    props.view_distance = value.parse().unwrap_or(props.view_distance)
                }
                "tick-distance" => {
                    props.tick_distance = value.parse().unwrap_or(props.tick_distance)
                }
                "xbox-auth" => props.xbox_auth = truthy,
                "auto-save" => props.auto_save = truthy,
                other => {
                    props.extra.insert(other.to_string(), value.clone());
                }
            }
        }
        if props.allow_list {
            props.white_list = true;
        }
        props
    }

    pub fn to_string_pretty(&self) -> String {
        let mut out = String::new();
        out.push_str("# PocketMine-Rust server configuration\n");
        let bool_str = |b: bool| if b { "on" } else { "off" };
        out.push_str(&format!("motd={}\n", self.motd));
        out.push_str(&format!("sub-motd={}\n", self.sub_motd));
        out.push_str(&format!("server-ip={}\n", self.server_ip));
        out.push_str(&format!("server-port={}\n", self.server_port));
        out.push_str(&format!("server-portv6={}\n", self.server_port_v6));
        out.push_str(&format!("enable-query={}\n", bool_str(self.enable_query)));
        out.push_str(&format!("query-port={}\n", self.query_port));
        out.push_str(&format!("gamemode={}\n", self.gamemode.as_str()));
        out.push_str(&format!(
            "force-gamemode={}\n",
            bool_str(self.force_gamemode)
        ));
        out.push_str(&format!("difficulty={}\n", self.difficulty));
        out.push_str(&format!("max-players={}\n", self.max_players));
        out.push_str(&format!("online-mode={}\n", bool_str(self.online_mode)));
        out.push_str(&format!("allow-list={}\n", bool_str(self.allow_list)));
        out.push_str(&format!("spawn-protection={}\n", self.spawn_protection));
        out.push_str(&format!("allow-flight={}\n", bool_str(self.allow_flight)));
        out.push_str(&format!(
            "enable-command-blocks={}\n",
            bool_str(self.enable_command_blocks)
        ));
        out.push_str(&format!("level-name={}\n", self.level_name));
        out.push_str(&format!("level-seed={}\n", self.level_seed));
        out.push_str(&format!("level-type={}\n", self.level_type));
        out.push_str(&format!("default-permission={}\n", self.default_permission));
        out.push_str(&format!(
            "player-idle-timeout={}\n",
            self.player_idle_timeout
        ));
        out.push_str(&format!(
            "compression-threshold={}\n",
            self.compression_threshold
        ));
        out.push_str(&format!(
            "compression-algorithm={}\n",
            self.compression_algorithm
        ));
        out.push_str(&format!("view-distance={}\n", self.view_distance));
        out.push_str(&format!("tick-distance={}\n", self.tick_distance));
        out.push_str(&format!("xbox-auth={}\n", bool_str(self.xbox_auth)));
        out.push_str(&format!("auto-save={}\n", bool_str(self.auto_save)));
        for (key, value) in &self.extra {
            out.push_str(&format!("{}={}\n", key, value));
        }
        out
    }

    pub fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, self.to_string_pretty());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.extra.get(key).map(String::as_str)
    }
}

/// Minimal `YAML` reader for `pocketmine.yml`, which only uses a flat
/// `key: value` structure plus nested sections. Nested values are flattened to
/// `section.key` for easy lookup.
pub struct Config {
    pub values: BTreeMap<String, String>,
}

impl Config {
    pub fn load_or_default(path: &Path) -> Self {
        let content = fs::read_to_string(path).unwrap_or_default();
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Self {
        let mut values = BTreeMap::new();
        let mut stack: Vec<(usize, String)> = Vec::new();
        for raw in content.lines() {
            let line = raw.trim_end();
            if line.trim().is_empty() || line.trim_start().starts_with('#') {
                continue;
            }
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim();
            let Some((key, value)) = trimmed.split_once(':') else {
                continue;
            };
            while matches!(stack.last(), Some((i, _)) if *i >= indent) {
                stack.pop();
            }
            let key = key.trim().to_string();
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            let mut full = String::new();
            for (_, parent) in &stack {
                full.push_str(parent);
                full.push('.');
            }
            full.push_str(&key);
            if value.is_empty() {
                stack.push((indent, key));
            } else {
                values.insert(full, value);
            }
        }
        Self { values }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key)
            .map(|v| v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("on") || v == "1")
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(|v| v.parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vanilla_properties() {
        let props =
            ServerProperties::parse("motd=Hello\nserver-port=19133\npvp=on\nlevel-name=test\n");
        assert_eq!(props.motd, "Hello");
        assert_eq!(props.server_port, 19133);
        assert_eq!(props.level_name, "test");
        assert_eq!(props.get("pvp"), Some("on"));
    }

    #[test]
    fn parses_nested_yaml() {
        let cfg = Config::parse("settings:\n  async-workers: 4\n  enabled: true\n");
        assert_eq!(cfg.get("settings.async-workers"), Some("4"));
        assert_eq!(cfg.get_bool("settings.enabled"), Some(true));
    }
}
