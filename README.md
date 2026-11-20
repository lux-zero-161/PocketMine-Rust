# PocketMine-Rust

A **1:1 port of [PocketMine-MP](https://github.com/pmmp/PocketMine-MP) to Rust**, with
**plugins written in C# (.NET 10)** that run in-process inside the Rust server.

* Engine: **Rust** (safe, fast, single-threaded world tick like PMMP, multi-threaded console/network I/O)
* Plugins: **C#** via the real CoreCLR (`hostfxr`), no sockets, no IPC
* Target: **Minecraft: Bedrock Edition 1.26.60**, protocol **2225**
* Plugin API: **PocketMine.Sdk 1.0.0.0**, modelled on PMMP's plugin API

The PHP reference lives in `../PocketMine-MP` (1,797 files / 168,434 lines).
Porting progress, namespace mapping and the milestone plan are in
[`PORTING.md`](PORTING.md).

---

## Status

PocketMine-Rust is a working foundation, not a finished server: the complete
stack runs (Rust core + in-process C# plugins), but a vanilla client cannot
join yet because Bedrock login/JWT/encryption are the next milestone.
LAN discovery and the RakNet handshake already work.

Verified by `./tools/smoke-test.sh`:

| Feature | State |
|---|---|
| Rust server with 20 TPS tick loop, console, log file | ✅ |
| Rust + C# task scheduler | ✅ |
| Block/item registry from `bedrock-data` (900+ blocks, 1,900+ items) | ✅ |
| Chunked flat world, palette sub-chunks, block get/set | ✅ |
| Players, inventory (36+4+1), gamemodes, permissions | ✅ |
| Player inventory sync from Rust to C# | ✅ |
| Admin commands: `/gamemode`, `/give`, `/tp`, `/kill` | ✅ |
| C# plugin loading, enable/disable, `/reload` (collectible contexts) | ✅ |
| C# events: priority, cancellation, **mutation back into Rust** | ✅ |
| C# commands, aliases, permissions | ✅ |
| C# scheduler, config, player/world/block/inventory API | ✅ |
| RakNet ping/pong (server appears in LAN list) | ✅ |
| RakNet offline handshake, ACKs, split frames | ✅ |
| Bedrock packet batches + raw-deflate compression | ✅ |
| Bedrock `NetworkSettings` + offline `Login` parsing | ✅ |
| ECDH P-384 handshake JWT (ES384) + AES-256-CFB8 encryption | ✅ |
| Encrypted `ClientToServerHandshake` → `LoginSuccess` | ✅ |
| `ResourcePacksInfo` / `ResourcePackStack` + client responses | ✅ |
| `RequestChunkRadius` → `ChunkRadiusUpdated`, `SetTime` | ✅ |
| Logged-in clients become real players (join/quit events) | ✅ |
| `StartGame`, chunk streaming, entities | ⏳ roadmap |

### Progress by subsystem

Roughly **5 %** of the 168,434 PHP lines are ported (6,800 lines Rust +
3,300 lines C#). Percentages are not linear: infrastructure is ahead of
gameplay and protocol coverage.

| Subsystem | Ported | Notes |
|---|---:|---|
| Build, workspace, logging, shutdown | 90 % | `build.sh`, `logs/server.log` |
| Tick loop + scheduler (Rust and C#) | 85 % | verified over 130 ticks |
| Configuration (`server.properties`, `pocketmine.yml`) | 75 % | typed keys, unknown keys preserved |
| Maths (Vector3, blocks, AABB, RNG) | 70 % | unit-tested |
| Block/item registry (`bedrock-data`) | 65 % | real hardness/resistance values |
| World (chunks, sections, block API) | 30 % | flat generator, no LevelDB yet |
| Player / inventory / gamemodes | 35 % | stacking, permissions attachments |
| Commands | 35 % | map, aliases, permissions, 19 built-ins |
| Events | 20 % | 16 core events, cancel + patch-back |
| C# plugin system | 65 % | load, lifecycle, reload, events, commands, tasks, config |
| Network (RakNet/Bedrock) | 45 % | ping, batches, split frames, compression, encrypted login, resource packs, chunk radius |
| Entities / AI | 5 % | model only, no physics or AI |
| Crafting / containers / forms | 0 % | planned (M3/M4) |
| World formats (LevelDB/Anvil), NBT | 0 % | planned (M2) |
| Resource packs, Query, RCON | 0 % | planned (M5) |

### Next milestones

1. **M1 Login:** ~~JWT parsing, `ServerToClientHandshake`, AES, compression,
   resource packs, chunk radius~~ – done. Next: `StartGame`, block/item
   palettes and chunk streaming. Until then the server disconnects cleanly
   after the resource pack stack.
2. **M2 World:** LevelDB read/write, block states, lighting, neighbour updates.
3. **M3 Gameplay:** entities, physics, damage/death, inventory transactions.
4. **M4 API parity:** all PMMP events, form API, command parser, selectors.
5. **M5 Operations:** timings, crash dumps, resource packs, Query/RCON, CI.

Namespace mapping, acceptance criteria and the porting workflow:
[`PORTING.md`](PORTING.md).

---

## Repository layout

```text
PocketMine-Rust/
├── crates/                     Rust engine
│   ├── pm_core/                constants, logger, server.properties, pocketmine.yml, text
│   ├── pm_math/                Vector2/3, BlockPosition, Facing, AABB, Location, RNG
│   ├── pm_data/                block/item registry (bedrock-data JSON, same data as PMMP)
│   ├── pm_event/               events, priorities, cancellation, JSON patch
│   ├── pm_permission/          permissions, defaults, attachments
│   ├── pm_command/             command map, aliases, senders
│   ├── pm_scheduler/           delayed / repeating tick tasks
│   ├── pm_world/               chunks, sub-chunks, blocks, worlds, entities
│   ├── pm_player/              player, inventory, gamemodes, player manager
│   ├── pm_network/             RakNet UDP, pong, handshake, frame codec
│   ├── pm_plugin_host/         hostfxr + C ABI + event/command bridge
│   └── pm_server/              bootstrap, tick loop, built-in commands, ServerBridge
├── csharp/
│   ├── PocketMine.Sdk/         API referenced by plugin authors
│   ├── PocketMine.Host/        managed host loaded by Rust (do not reference directly)
│   └── plugins/ExamplePlugin/  reference plugin (events, commands, scheduler, world API)
├── plugins/                    drop compiled plugins here (also created on start)
├── tools/                      ping.py, smoke-test.sh
├── server.properties           Bedrock server configuration
├── pocketmine.yml              engine configuration
└── PORTING.md                  port status + roadmap
```

---

## Build & run

Requirements: Rust 1.75+ (tested with 1.100), the **.NET 10 SDK** and
**OpenSSL** development headers (used for ECDH/ES384/AES, like PMMP's
`ext-openssl`).

```bash
./build.sh      # C# SDK + host + example plugin + Rust release binary
./start.sh      # start the server
```

Windows: `build.cmd`, then `start.cmd` / `start.ps1`.

Useful flags:

```text
--no-plugins        skip the C# host (pure Rust mode)
--no-network        skip the UDP listener
--port <port>       override server-port
--plugins <dir>     plugin directory (default: pocketmine.yml / "plugins")
--ticks <n>         stop after n ticks (CI/smoke tests)
--version / --help
```

On start the server also prints the level, binds UDP (19132) and advertises
itself via RakNet:

```bash
python3 tools/ping.py 19132
```

---

## Writing a C# plugin

### 1. Project

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
    <Version>1.0.0.0</Version>
  </PropertyGroup>
  <ItemGroup>
    <ProjectReference Include="..\..\PocketMine.Sdk\PocketMine.Sdk.csproj" />
  </ItemGroup>
</Project>
```

### 2. Plugin class

```csharp
using PocketMine.Plugin;

namespace MyPlugin;

[Plugin("MyPlugin", "1.0.0", Author = "You", Api = "1.0.0.0",
        Description = "Example plugin")]
public class Main : PluginBase
{
    public override void OnLoad()
    {
        Logger.Info("Loading...");
        if (!Config.Has("message"))
        {
            Config.Set("message", "§aHello, {player}!");
            SaveConfig();               // plugins/MyPlugin/config.yml
        }
    }

    public override void OnEnable()
    {
        Server.RegisterCommand(new HelloCommand());

        // Rust tick scheduler: every 5 seconds
        Server.Scheduler.RunTaskTimer(this, () =>
            Logger.Info($"TPS {Server.Tps:F1}, players {Server.PlayerCount}"),
            delayTicks: 20, periodTicks: 100);
    }

    public override void OnDisable() => Logger.Info("Bye.");
}
```

### 3. Commands

```csharp
using PocketMine.Command;

public class HelloCommand : Command
{
    public HelloCommand() : base("hello", "Greets you", "/hello", "myplugin.command.hello")
    {
        Aliases.Add("hi");
    }

    public override bool Execute(ICommandSender sender, string label, string[] args)
    {
        sender.SendMessage("§aHello from C# inside Rust!");
        return true;
    }
}
```

### 4. Events

```csharp
using PocketMine.Event;

public class Listener : IListener
{
    [EventHandler(Priority = EventPriority.Normal)]
    public void OnJoin(PlayerJoinEvent ev)
    {
        ev.JoinMessage = $"§e{ev.Player.Name} joined §7(C#)";   // visible in Rust
        ev.Player.SendMessage($"§aWelcome, {ev.Player.Name}!");
    }

    [EventHandler(Priority = EventPriority.High)]
    public void OnChat(PlayerChatEvent ev)
    {
        if (ev.Message == "!ping")
        {
            ev.Player.SendMessage("§bPong from C#!");
            ev.IsCancelled = true;
        }
    }
}
```

Register listeners in `OnEnable()`:

```csharp
Server.RegisterEvents(new Listener(), this);
```

### 5. World / player API

```csharp
Player? player = Server.GetPlayer("Steve");
World? world = Server.DefaultWorld;

Block block = world.GetBlock(0, 64, 0);
world.SetBlock(0, 65, 0, "minecraft:stone");

player?.Teleport(0, 80, 0);
Server.BroadcastMessage($"§e{Server.PlayerCount} players online");
```

### 6. Packaging

* `plugins/MyPlugin.dll` (single assembly, simplest), or
* `plugins/MyPlugin/MyPlugin.dll` plus dependencies and an optional
  `plugin.json`:

```json
{
  "name": "MyPlugin",
  "version": "1.0.0",
  "main": "MyPlugin.Main",
  "api": "1.0.0.0",
  "author": "You",
  "description": "Example",
  "depend": []
}
```

The loader resolves `PocketMine.Sdk` from the host, other assemblies from the
plugin folder, and supports dependency ordering via `Depends`/`depend`.

---

## Built-in commands

| Command | Description |
|---|---|
| `/help [command]` | command list and details |
| `/info`, `/status`, `/version`, `/tps` | server, world and performance info |
| `/plugins` | loaded C# plugins |
| `/list`, `/say <msg>`, `/kick <player> [reason]`, `/op`, `/deop` | administration |
| `/gamemode <mode> [player]`, `/give <player> <item> [count]` | player state |
| `/tp <player> <x> <y> <z>`, `/kill <player>` | movement and damage |
| `/world <info\|list\|time\|settime\|getblock\|setblock\|tp>` | world and block access |
| `/event <join\|quit\|chat\|break\|place\|damage\|death\|respawn>` | dispatch a test event to plugins |
| `/reload` | disable, reload and re-enable plugins |
| `/stop` | save worlds and shut down |
| `/csharp`, `/hello`, `/build`, `/inv` | provided by `ExamplePlugin` |

`/event` exists so the whole Rust ↔ C# event pipeline can be tested without a
Bedrock client.

---

## C# API overview

| Namespace | Contents |
|---|---|
| `PocketMine.Plugin` | `PluginBase`, `[Plugin]`, config, resources |
| `PocketMine.Server` | `IServer`, `IPluginManager`, `ServerInfo`, `NativeBridge` |
| `PocketMine.Player` | `Player`, `GameMode` |
| `PocketMine.World` | `World`, `Block` |
| `PocketMine.Item` / `.Inventory` | `ItemStack`, `PlayerInventory` |
| `PocketMine.Event` | `Event`, `CancellableEvent`, `IListener`, `[EventHandler]`, 16 events |
| `PocketMine.Command` | `Command`, `ICommandSender`, `ConsoleCommandSender` |
| `PocketMine.Scheduler` | `ITaskScheduler`, `TaskHandler` |
| `PocketMine.Permission` | `Permission`, `PermissionDefault` |
| `PocketMine.Math` | `Vector3`, `BlockPosition` |
| `PocketMine.Utils` | `ILogger`, `Config`, `TextFormat` |

---

## Tests

```bash
cargo test --workspace      # registry, world, inventory, scheduler, pong, frames
./tools/smoke-test.sh       # builds everything and drives the server end-to-end
```

The smoke test asserts that the C# plugin loads, receives commands/events,
mutates chat/join messages, uses the world API, runs scheduled tasks and that
the UDP listener answers a RakNet ping.

---

## License

LGPL-3.0, matching PocketMine-MP. See [`LICENSE`](LICENSE).
