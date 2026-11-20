use pm_command::{Command, CommandSender, SimpleCommandMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct HelpCommand {
    command_map: Arc<SimpleCommandMap>,
}

impl HelpCommand {
    pub fn new(command_map: Arc<SimpleCommandMap>) -> Self {
        Self { command_map }
    }
}

impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "Shows the help menu"
    }

    fn usage(&self) -> &str {
        "/help [page|command]"
    }

    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        let commands = self.command_map.get_all_commands();
        sender.send_message("\x1b[33m--- Showing help page (all commands) ---\x1b[0m");
        for cmd in commands {
            sender.send_message(&format!(
                "\x1b[32m/{}\x1b[0m: {}",
                cmd.name(),
                cmd.description()
            ));
        }
        true
    }
}

pub struct StopCommand {
    running: Arc<AtomicBool>,
}

impl StopCommand {
    pub fn new(running: Arc<AtomicBool>) -> Self {
        Self { running }
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

    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        sender.send_message("\x1b[33mStopping the server...\x1b[0m");
        self.running.store(false, Ordering::Relaxed);
        true
    }
}

pub struct VersionCommand;

impl Command for VersionCommand {
    fn name(&self) -> &str {
        "version"
    }

    fn description(&self) -> &str {
        "Gets the version of this server including plugins"
    }

    fn usage(&self) -> &str {
        "/version"
    }

    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        sender.send_message(&format!(
            "\x1b[36mThis server is running {} version {} \"{}\" (Bedrock Edition protocol {})\x1b[0m",
            pm_core::NAME,
            pm_core::VERSION,
            pm_core::CODENAME,
            pm_core::CURRENT_PROTOCOL
        ));
        sender.send_message("\x1b[36mPlugin engine powered by Microsoft .NET 10 (C#)\x1b[0m");
        true
    }
}

pub struct SayCommand;

impl Command for SayCommand {
    fn name(&self) -> &str {
        "say"
    }

    fn description(&self) -> &str {
        "Broadcasts the given message as the sender"
    }

    fn usage(&self) -> &str {
        "/say <message ...>"
    }

    fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
        if args.is_empty() {
            sender.send_message(&format!("Usage: {}", self.usage()));
            return false;
        }
        let msg = args.join(" ");
        sender.send_message(&format!("\x1b[35m[{}] {}\x1b[0m", sender.name(), msg));
        true
    }
}

pub struct ListCommand;

impl Command for ListCommand {
    fn name(&self) -> &str {
        "list"
    }

    fn description(&self) -> &str {
        "Lists all online players"
    }

    fn usage(&self) -> &str {
        "/list"
    }

    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        sender.send_message("There are 0/20 players online.");
        true
    }
}

pub struct PluginsCommand {
    plugins_dir: String,
}

impl PluginsCommand {
    pub fn new(plugins_dir: impl Into<String>) -> Self {
        Self {
            plugins_dir: plugins_dir.into(),
        }
    }
}

impl Command for PluginsCommand {
    fn name(&self) -> &str {
        "plugins"
    }

    fn description(&self) -> &str {
        "Gets a list of plugins running on the server"
    }

    fn usage(&self) -> &str {
        "/plugins"
    }

    fn execute(&self, sender: &dyn CommandSender, _label: &str, _args: &[&str]) -> bool {
        let p = std::path::Path::new(&self.plugins_dir);
        let mut plugin_names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    if ext == "dll" {
                        if let Some(stem) = entry.path().file_stem() {
                            plugin_names.push(format!("\x1b[32m{}\x1b[0m", stem.to_string_lossy()));
                        }
                    }
                }
            }
        }

        if plugin_names.is_empty() {
            sender.send_message("Plugins (0): None");
        } else {
            sender.send_message(&format!("Plugins ({}): {}", plugin_names.len(), plugin_names.join(", ")));
        }
        true
    }
}
