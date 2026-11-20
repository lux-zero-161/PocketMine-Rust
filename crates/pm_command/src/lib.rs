use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub trait CommandSender: Send + Sync {
    fn name(&self) -> &str;
    fn send_message(&self, message: &str);
    fn has_permission(&self, permission: &str) -> bool;
    fn is_player(&self) -> bool {
        false
    }
    fn is_op(&self) -> bool {
        self.has_permission("pocketmine.command.op")
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

    fn is_op(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone)]
pub struct CommandMeta {
    pub name: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub usage: String,
    pub permission: Option<String>,
    pub permission_message: Option<String>,
    pub plugin: Option<String>,
}

pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn usage(&self) -> &str;
    fn aliases(&self) -> &[String] {
        &[]
    }
    fn permission(&self) -> Option<&str> {
        None
    }
    fn plugin(&self) -> Option<&str> {
        None
    }
    fn test_permission(&self, sender: &dyn CommandSender) -> bool {
        match self.permission() {
            Some(permission) => sender.has_permission(permission),
            None => true,
        }
    }
    fn execute(&self, sender: &dyn CommandSender, label: &str, args: &[&str]) -> bool;
}

pub struct SimpleCommandMap {
    commands: RwLock<HashMap<String, Arc<dyn Command>>>,
    aliases: RwLock<HashMap<String, String>>,
}

impl SimpleCommandMap {
    pub fn new() -> Self {
        Self {
            commands: RwLock::new(HashMap::new()),
            aliases: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(&self, command: Arc<dyn Command>) {
        let name = command.name().to_ascii_lowercase();
        let aliases = command.aliases().to_vec();
        {
            let mut map = self.commands.write().unwrap();
            map.insert(name.clone(), command);
        }
        let mut alias_map = self.aliases.write().unwrap();
        for alias in aliases {
            alias_map.insert(alias.to_ascii_lowercase(), name.clone());
        }
    }

    /// Registers a command and records its aliases.
    pub fn register_with_aliases(&self, command: Arc<dyn Command>, aliases: Vec<String>) {
        let name = command.name().to_ascii_lowercase();
        {
            let mut map = self.commands.write().unwrap();
            map.insert(name.clone(), command);
        }
        let mut alias_map = self.aliases.write().unwrap();
        for alias in aliases {
            alias_map.insert(alias.to_ascii_lowercase(), name.clone());
        }
    }

    pub fn unregister(&self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        let removed = self.commands.write().unwrap().remove(&name).is_some();
        let mut aliases = self.aliases.write().unwrap();
        aliases.retain(|_, target| target != &name);
        removed
    }

    /// Removes all commands that were registered by a plugin.
    pub fn unregister_plugin(&self, plugin: &str) {
        let names: Vec<String> = self
            .commands
            .read()
            .unwrap()
            .values()
            .filter(|c| c.plugin() == Some(plugin))
            .map(|c| c.name().to_ascii_lowercase())
            .collect();
        for name in names {
            self.unregister(&name);
        }
    }

    pub fn get_command(&self, name: &str) -> Option<Arc<dyn Command>> {
        let key = name.to_ascii_lowercase();
        if let Some(cmd) = self.commands.read().unwrap().get(&key) {
            return Some(cmd.clone());
        }
        let aliases = self.aliases.read().unwrap();
        let target = aliases.get(&key)?;
        self.commands.read().unwrap().get(target).cloned()
    }

    pub fn exists(&self, name: &str) -> bool {
        self.get_command(name).is_some()
    }

    pub fn get_all_commands(&self) -> Vec<Arc<dyn Command>> {
        self.commands.read().unwrap().values().cloned().collect()
    }

    pub fn get_all_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.commands.read().unwrap().keys().cloned().collect();
        names.sort();
        names
    }

    pub fn aliases_of(&self, name: &str) -> Vec<String> {
        let target = name.to_ascii_lowercase();
        self.aliases
            .read()
            .unwrap()
            .iter()
            .filter(|(_, cmd)| *cmd == &target)
            .map(|(alias, _)| alias.clone())
            .collect()
    }

    /// Parses and executes a command line. Returns `false` when the command is
    /// unknown or the sender lacks permission, exactly like PMMP.
    pub fn execute(&self, sender: &dyn CommandSender, command_line: &str) -> bool {
        let trimmed = command_line.trim().trim_start_matches('/');
        if trimmed.is_empty() {
            return false;
        }

        // Split into the label and the raw argument tail so quoted strings keep
        // their content.
        let (label, rest) = match trimmed.split_once(char::is_whitespace) {
            Some((label, rest)) => (label, rest.trim()),
            None => (trimmed, ""),
        };

        let Some(command) = self.get_command(label) else {
            sender.send_message(&format!(
                "§cUnknown command: '{}'. Try /help for a list of commands.",
                label
            ));
            return false;
        };

        if let Some(permission) = command.permission() {
            if !sender.has_permission(permission) {
                sender.send_message(&format!(
                    "§cYou don't have permission to use this command ({}).",
                    permission
                ));
                return false;
            }
        }

        let args: Vec<&str> = if rest.is_empty() {
            Vec::new()
        } else {
            rest.split_whitespace().collect()
        };

        command.execute(sender, label, &args)
    }
}

impl Default for SimpleCommandMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Splits a command line into arguments while honouring double quotes, the way
/// Bedrock's command parser handles player names containing spaces.
pub fn split_arguments(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in input.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSender;
    impl CommandSender for TestSender {
        fn name(&self) -> &str {
            "test"
        }
        fn send_message(&self, _message: &str) {}
        fn has_permission(&self, _permission: &str) -> bool {
            true
        }
    }

    struct Echo;
    impl Command for Echo {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "echo"
        }
        fn usage(&self) -> &str {
            "/echo <text>"
        }
        fn aliases(&self) -> &[String] {
            const ALIASES: &[String] = &[];
            ALIASES
        }
        fn execute(&self, sender: &dyn CommandSender, _label: &str, args: &[&str]) -> bool {
            sender.send_message(&args.join(" "));
            true
        }
    }

    #[test]
    fn executes_commands_and_aliases() {
        let map = SimpleCommandMap::new();
        map.register(Arc::new(Echo));
        map.register_with_aliases(map.get_command("echo").unwrap(), vec!["say2".into()]);
        assert!(map.execute(&TestSender, "/echo hello world"));
        assert!(map.execute(&TestSender, "say2 hi"));
        assert!(map.get_command("say2").is_some());
        assert!(!map.execute(&TestSender, "nope"));
    }

    #[test]
    fn splits_quotes() {
        assert_eq!(split_arguments(r#"a "b c" d"#), vec!["a", "b c", "d"]);
    }
}
