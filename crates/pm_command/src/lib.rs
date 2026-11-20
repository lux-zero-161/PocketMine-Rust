use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub trait CommandSender: Send + Sync {
    fn name(&self) -> &str;
    fn send_message(&self, message: &str);
    fn has_permission(&self, permission: &str) -> bool;
    fn is_player(&self) -> bool {
        false
    }
}

pub struct ConsoleCommandSender {
    logger: Arc<pm_core::Logger>,
}

impl ConsoleCommandSender {
    pub fn new(logger: Arc<pm_core::Logger>) -> Self {
        Self { logger }
    }
}

impl CommandSender for ConsoleCommandSender {
    fn name(&self) -> &str {
        "CONSOLE"
    }

    fn send_message(&self, message: &str) {
        self.logger.info(message);
    }

    fn has_permission(&self, _permission: &str) -> bool {
        true
    }
}

pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn usage(&self) -> &str;
    fn permission(&self) -> Option<&str> {
        None
    }
    fn execute(&self, sender: &dyn CommandSender, label: &str, args: &[&str]) -> bool;
}

pub struct SimpleCommandMap {
    commands: RwLock<HashMap<String, Arc<dyn Command>>>,
}

impl SimpleCommandMap {
    pub fn new() -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, command: Arc<dyn Command>) {
        let mut map = self.commands.write().unwrap();
        map.insert(command.name().to_lowercase(), command);
    }

    pub fn unregister(&self, name: &str) {
        let mut map = self.commands.write().unwrap();
        map.remove(&name.to_lowercase());
    }

    pub fn get_command(&self, name: &str) -> Option<Arc<dyn Command>> {
        let map = self.commands.read().unwrap();
        map.get(&name.to_lowercase()).cloned()
    }

    pub fn get_all_commands(&self) -> Vec<Arc<dyn Command>> {
        let map = self.commands.read().unwrap();
        map.values().cloned().collect()
    }

    pub fn execute(&self, sender: &dyn CommandSender, command_line: &str) -> bool {
        let trimmed = command_line.trim().trim_start_matches('/');
        if trimmed.is_empty() {
            return false;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let cmd_name = parts[0];
        let args = &parts[1..];

        if let Some(cmd) = self.get_command(cmd_name) {
            if let Some(perm) = cmd.permission() {
                if !sender.has_permission(perm) {
                    sender.send_message("\x1b[31mYou don't have permission to use this command.\x1b[0m");
                    return false;
                }
            }
            cmd.execute(sender, cmd_name, args)
        } else {
            sender.send_message(&format!("\x1b[31mUnknown command: '{}'. Try /help for a list of commands.\x1b[0m", cmd_name));
            false
        }
    }
}

impl Default for SimpleCommandMap {
    fn default() -> Self {
        Self::new()
    }
}
