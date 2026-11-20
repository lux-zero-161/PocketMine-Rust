mod commands;

use commands::*;
use pm_command::{ConsoleCommandSender, SimpleCommandMap};
use pm_core::{Logger, ServerProperties};
use pm_network::BedrockNetworkServer;
use pm_plugin_host::CSharpPluginManager;
use pm_world::World;
use std::io::{self, BufRead};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn print_banner() {
    println!("\x1b[38;5;39m");
    println!(r#"   ____             _        _   __  __ _               ____            _   "#);
    println!(r#"  |  _ \ ___   ____| | _____| |_|  \/  (_)_ __   ___   |  _ \ _   _ ___| |_ "#);
    println!(r#"  | |_) / _ \ / ___| |/ / _ \ __| |\/| | | '_ \ / _ \  | |_) | | | / __| __|"#);
    println!(r#"  |  __/ (_) | |___|   <  __/ |_| |  | | | | | |  __/  |  _ <| |_| \__ \ |_ "#);
    println!(r#"  |_|   \___/ \____|_|\_\___|\__|_|  |_|_|_| |_|\___|  |_| \_\\__,_|___/\__|"#);
    println!("\x1b[0m");
}

fn main() {
    print_banner();

    let logger = Arc::new(Logger::default());
    logger.info(&format!("Starting {} version {}", pm_core::NAME, pm_core::VERSION));
    logger.info(&format!("Minecraft: Bedrock Edition protocol v{} (v{})", pm_core::CURRENT_PROTOCOL, pm_core::MINECRAFT_VERSION));

    // Load server.properties
    let props_path = Path::new("server.properties");
    let props = ServerProperties::load_or_create(props_path);
    logger.info(&format!("Loaded server configuration: MOTD=\"{}\", Port={}", props.motd, props.server_port));

    // Command Map & Console Sender
    let command_map = Arc::new(SimpleCommandMap::new());
    let console_sender = Arc::new(ConsoleCommandSender::new(logger.clone()));

    let running = Arc::new(AtomicBool::new(true));

    // Register default commands
    command_map.register(Arc::new(HelpCommand::new(command_map.clone())));
    command_map.register(Arc::new(StopCommand::new(running.clone())));
    command_map.register(Arc::new(VersionCommand));
    command_map.register(Arc::new(SayCommand));
    command_map.register(Arc::new(ListCommand));
    command_map.register(Arc::new(PluginsCommand::new("plugins")));

    // World system
    let default_world = Arc::new(World::new(&props.level_name));
    logger.info(&format!("Preparing level \"{}\"...", props.level_name));

    // Initialize C# Plugin Subsystem
    logger.info("Initializing Microsoft .NET C# Plugin Engine...");
    let plugin_manager = match CSharpPluginManager::new(logger.clone(), command_map.clone()) {
        Ok(mgr) => {
            let loaded = mgr.load_plugins("plugins");
            logger.info(&format!("Discovered and loaded {} C# plugin assembly(s)", loaded));
            mgr.enable_plugins();
            Some(mgr)
        }
        Err(err) => {
            logger.warning(&format!("C# Plugin Engine failed to start: {}", err));
            logger.warning("Server running in pure Rust mode without C# plugins.");
            None
        }
    };

    // Bedrock Network Listener (RakNet UDP)
    let bind_addr = format!("0.0.0.0:{}", props.server_port);
    let mut network_server = match BedrockNetworkServer::bind(&bind_addr, &props.motd, props.max_players as usize, logger.clone()) {
        Ok(server) => {
            logger.info(&format!("Network interface bound to UDP {}", bind_addr));
            Some(server)
        }
        Err(e) => {
            logger.error(&format!("Failed to bind UDP port {}: {}", props.server_port, e));
            None
        }
    };

    logger.info(&format!("Server done! (For help, type \"help\" or \"?\")"));

    // Spawn Console Input Reader Thread
    let cmd_map_clone = command_map.clone();
    let console_sender_clone = console_sender.clone();
    let running_clone = running.clone();
    let logger_clone = logger.clone();

    thread::spawn(move || {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if !running_clone.load(Ordering::Relaxed) {
                break;
            }
            match line {
                Ok(cmd) => {
                    let trimmed = cmd.trim();
                    if !trimmed.is_empty() {
                        cmd_map_clone.execute(&*console_sender_clone, trimmed);
                    }
                }
                Err(_) => break,
            }
        }
    });

    // 20 TPS Server Tick Loop (50ms per tick)
    let tick_duration = Duration::from_millis(pm_core::TICK_DURATION_MS);
    let mut current_tick: u64 = 0;

    while running.load(Ordering::Relaxed) {
        let tick_start = Instant::now();

        // 1. Tick Network Packets
        if let Some(ref mut net) = network_server {
            net.process_packets();
        }

        // 2. Tick World
        default_world.tick();

        current_tick += 1;

        // Sleep remaining time in tick
        let elapsed = tick_start.elapsed();
        if elapsed < tick_duration {
            thread::sleep(tick_duration - elapsed);
        } else if elapsed > tick_duration * 2 {
            logger_clone.debug(&format!("Server tick #{} took {:?} (lag detected)", current_tick, elapsed));
        }
    }

    // Graceful Shutdown
    logger.info("Stopping server...");

    if let Some(net) = network_server {
        net.stop();
    }

    if let Some(mgr) = plugin_manager {
        logger.info("Disabling C# plugins...");
        mgr.disable_plugins();
    }

    logger.info("Saving worlds...");
    logger.info("Server stopped.");
}
