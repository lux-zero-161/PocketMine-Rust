mod commands;
mod player_commands;
mod server;

use pm_core::{text, Logger, ServerProperties};
use pm_plugin_host::{PluginHost, ServerBridge};
use pm_scheduler::Task;
use server::ServerState;
use std::io::{self, BufRead};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn print_banner(logger: &Logger) {
    logger.info("§b   ____             _        _   __  __ _");
    logger.info("§b  |  _ \\ ___   ____| | _____| |_|  \\/  (_)_ __   ___");
    logger.info("§b  | |_) / _ \\ / ___| |/ / _ \\ __| |\\/| | | '_ \\ / _ \\");
    logger.info("§b  |  __/ (_) | |___|   <  __/ |_| |  | | | | | |  __/");
    logger.info("§b  |_|   \\___/ \\____|_|\\_\\___|\\__|_|  |_|_|_| |_|\\___|");
    logger.info(&format!(
        "§7Rust core {} §8| §7Minecraft Bedrock {} (protocol {})",
        pm_core::VERSION,
        pm_core::MINECRAFT_VERSION,
        pm_core::CURRENT_PROTOCOL
    ));
}

fn print_help() {
    println!("PocketMine-Rust {}", pm_core::VERSION);
    println!();
    println!("Usage: PocketMine [options]");
    println!();
    println!("  --no-plugins          Do not start the C# plugin host");
    println!("  --no-network          Do not bind the UDP listener");
    println!("  --port <port>         Override server-port");
    println!("  --plugins <dir>       Plugin directory (default: plugins)");
    println!("  --ticks <n>           Stop automatically after n ticks (testing)");
    println!("  --version             Print version information");
    println!("  --help                Show this help");
}

struct Options {
    plugins: bool,
    network: bool,
    port: Option<u16>,
    plugins_dir: Option<String>,
    ticks: Option<u64>,
}

fn parse_options() -> Result<Options, String> {
    let mut options = Options {
        plugins: true,
        network: true,
        port: None,
        plugins_dir: None,
        ticks: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--no-plugins" | "--no-csharp" => options.plugins = false,
            "--no-network" => options.network = false,
            "--port" => {
                let value = args.next().ok_or("--port needs a value")?;
                options.port = Some(value.parse().map_err(|_| "--port needs a number")?);
            }
            "--plugins" => {
                options.plugins_dir = Some(args.next().ok_or("--plugins needs a directory")?)
            }
            "--ticks" => {
                let value = args.next().ok_or("--ticks needs a number")?;
                options.ticks = Some(value.parse().map_err(|_| "--ticks needs a number")?);
            }
            "--version" | "-v" => {
                println!("{} {}", pm_core::NAME, pm_core::VERSION);
                println!("PocketMine-MP base: {}", pm_core::BASE_VERSION);
                println!(
                    "Minecraft: {} (protocol {})",
                    pm_core::MINECRAFT_VERSION,
                    pm_core::CURRENT_PROTOCOL
                );
                println!("Plugin API: {}", pm_core::API_VERSION);
                std::process::exit(0);
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    Ok(options)
}

fn main() {
    let options = match parse_options() {
        Ok(options) => options,
        Err(err) => {
            eprintln!("{err}");
            print_help();
            std::process::exit(2);
        }
    };

    let logger = Arc::new(Logger::new("Server thread"));
    if let Err(err) = logger.attach_file(format!("{}/server.log", pm_core::LOG_DIRECTORY)) {
        logger.warning(&format!("Could not open log file: {}", err));
    }

    print_banner(&logger);

    let mut props = ServerProperties::load_or_create(std::path::Path::new("server.properties"));
    if let Some(port) = options.port {
        props.server_port = port;
    }
    logger.info(&format!(
        "Config: MOTD=\"{}\", port={}, level=\"{}\", max-players={}",
        props.motd, props.server_port, props.level_name, props.max_players
    ));

    let engine_config = pm_core::Config::load_or_default(std::path::Path::new("pocketmine.yml"));
    let plugins_dir = options
        .plugins_dir
        .clone()
        .or_else(|| {
            engine_config
                .get("csharp-plugins.plugins-folder")
                .map(str::to_string)
        })
        .unwrap_or_else(|| pm_core::PLUGIN_DIRECTORY.to_string());
    logger.info(&format!(
        "Engine config: pocketmine.yml ({} keys), plugin folder '{}'",
        engine_config.values.len(),
        plugins_dir
    ));

    let state = ServerState::new(logger.clone(), Arc::new(props));
    state.prepare_default_world();
    commands::register_default_commands(&state);

    // Register a repeating core task to prove the scheduler path works.
    {
        let mut scheduler = state.scheduler.lock().unwrap();
        let tick_ref = state.tick.load(Ordering::Relaxed);
        scheduler.schedule_repeating(
            Some("core".to_string()),
            20,
            1200,
            Box::new(|_tick| {}) as Box<dyn Task>,
            tick_ref,
        );
    }

    let mut plugin_host: Option<Arc<PluginHost>> = None;
    if options.plugins {
        logger.info("Starting C# plugin host (.NET CoreCLR)...");
        let bridge: Arc<dyn ServerBridge> = state.clone();
        match PluginHost::new(
            logger.clone(),
            state.commands.clone(),
            state.permissions.clone(),
            bridge,
        ) {
            Ok(host) => {
                let loaded = host.load_plugins(&plugins_dir);
                host.enable_plugins();
                logger.info(&format!(
                    "C# plugin subsystem ready ({} plugin(s) from '{}')",
                    loaded, plugins_dir
                ));
                let _ = state.plugin_host.set(host.clone());
                plugin_host = Some(host);
            }
            Err(err) => {
                logger.warning(&format!("C# plugin host unavailable: {}", err));
                logger.warning(
                    "Continuing with the pure-Rust core; /reload will retry after a build.",
                );
            }
        }
    } else {
        logger.info("C# plugin host disabled by --no-plugins");
    }

    state.dispatch(&mut pm_event::ServerStartEvent::new());

    let mut network = None;
    if options.network {
        let bind = format!("{}:{}", state.props.server_ip, state.props.server_port);
        match pm_network::BedrockNetworkServer::bind(
            &bind,
            &state.props.motd,
            state.props.max_players as usize,
            logger.clone(),
        ) {
            Ok(server) => {
                let server = Arc::new(server);
                *state.network.lock().unwrap() = Some(server.clone());
                network = Some(server);
            }
            Err(err) => {
                logger.error(&format!("Could not bind {}: {}", bind, err));
            }
        }
    } else {
        logger.info("UDP listener disabled by --no-network");
    }

    logger.info("Server started. Type \"help\" for commands.");

    // Console reader.
    {
        let state = state.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                if !state.is_running() {
                    break;
                }
                match line {
                    Ok(line) => {
                        let command = line.trim();
                        if command.is_empty() {
                            continue;
                        }
                        state.commands.execute(&*state.console, command);
                    }
                    Err(_) => break,
                }
            }
        });
    }

    let tick_duration = Duration::from_millis(pm_core::TICK_DURATION_MS);
    let mut last_warning = Instant::now();

    while state.is_running() {
        let tick_start = Instant::now();
        let tick = state.tick.fetch_add(1, Ordering::Relaxed) + 1;

        if let Some(max_ticks) = options.ticks {
            if tick >= max_ticks {
                logger.info(&format!("Reached --ticks limit ({}), stopping", max_ticks));
                state.shutdown();
            }
        }

        if let Some(server) = network.as_ref() {
            server.process_packets();
            server.set_online_count(state.players.count() as u32);
        }

        state.worlds.tick_all();
        state.scheduler.lock().unwrap().tick(tick);
        if let Some(host) = plugin_host.as_ref() {
            host.tick_scheduler(tick);
        }
        state.flush_player_messages();

        let elapsed = tick_start.elapsed();
        state.record_tick(elapsed.as_secs_f64() * 1000.0);

        if elapsed > tick_duration * 2 && last_warning.elapsed() > Duration::from_secs(5) {
            logger.warning(&format!(
                "Tick #{} took {:.1}ms (running behind)",
                tick,
                elapsed.as_secs_f64() * 1000.0
            ));
            last_warning = Instant::now();
        }

        if elapsed < tick_duration {
            thread::sleep(tick_duration - elapsed);
        }
    }

    logger.info("Stopping server...");
    state.dispatch(&mut pm_event::ServerStopEvent::new());

    if let Some(host) = plugin_host.as_ref() {
        logger.info("Disabling C# plugins...");
        host.disable_plugins();
    }
    logger.info("Saving worlds...");
    state.save_worlds();

    if let Some(server) = network.as_ref() {
        server.stop();
    }

    logger.info(&format!(
        "Server stopped after {} tick(s). {}",
        state.tick.load(Ordering::Relaxed),
        text::strip_colors("§aGoodbye!")
    ));
}
