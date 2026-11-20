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

## Fortschritt (Stand: 20.11.2026)

**Kurzfassung:** Der komplette Stack läuft – Rust-Core + In-Process-C#-Plugins.
Ein Vanilla-Client kann noch **nicht joinen**, weil Login/JWT/Verschlüsselung
fehlen; LAN-Erkennung und RakNet-Handshake funktionieren bereits.

Flächenmäßig sind rund **5 %** der PMMP-Codebasis portiert
(6.500 Zeilen Rust + 3.100 Zeilen C# gegenüber 168.434 Zeilen PHP). Das ist
kein linearer Maßstab: die Basis-Infrastruktur ist deutlich weiter als die
Prozentzahl, Gameplay/Netzwerk-Login noch nicht.

| Subsystem | Fortschritt | Stand |
|---|---:|---|
| Build, Workspace, Start/Stop, Logging | 90 % | `cargo build`, `./build.sh`, `logs/server.log` |
| Tick-Loop (20 TPS), Scheduler (Rust + C#) | 85 % | verifiziert über 130 Ticks, wiederholende Tasks |
| Konfiguration (`server.properties`, `pocketmine.yml`) | 75 % | typisierte Keys + unbekannte Keys bleiben erhalten |
| Mathematik (Vector3, Blocks, AABB, RNG) | 70 % | Unit-Tests |
| Block-/Item-Registry (`bedrock-data`) | 65 % | 900+ Blöcke / 1.900+ Items, echte Härtewerte |
| Welt (Chunks, Subchunks, Block-API) | 30 % | Flat-Generator, Paletten, get/set, kein LevelDB |
| Spieler / Inventar / Gamemodes | 35 % | 36+4+1 Slots, Stacking, Rechte-Attachments |
| Commands | 30 % | Map, Aliase, Permissions, 15 Built-ins |
| Events | 20 % | 16 Kern-Events, Prioritäten, Cancel, Patch-Back |
| C#-Plugin-System | 65 % | Laden, Lifecycle, `/reload`, Events, Commands, Tasks, Config |
| Netzwerk (RakNet/Bedrock) | 20 % | Ping/Pong, Offline-Handshake, Frames, `NetworkSettings` |
| Entities / KI | 5 % | Basismodell, keine Physik/KI |
| Crafting / Container / Formulare | 0 % | geplant (M3/M4) |
| Weltformate (LevelDB/Anvil), NBT | 0 % | geplant (M2) |
| Resource-Packs, Query, RCON | 0 % | geplant (M5) |

### Was jetzt nachweislich läuft

* 20-TPS-Server mit Console, Logdatei und sauberem Shutdown.
* C#-Plugin `ExamplePlugin` wird geladen, aktiviert und deaktiviert.
* C#-Events erhalten Priorität, können abbrechen **und Werte zurückschreiben**
  (z. B. Join-Nachricht, Chat-Text, Schaden).
* C#-Commands mit Aliassen und Permissions (`/csharp`, `/hello`, `/build`).
* C#-Scheduler läuft im Server-Takt; `Config` schreibt `config.yml`.
* Welt-API aus C#: Blöcke lesen/schreiben, Spieler teleportieren, Broadcast.
* RakNet-Ping liefert einen korrekten 1.26.60-Pong (Server erscheint im LAN).
* RakNet-Handshake (Request 1/2) und Bedrock-`NetworkSettings` antworten.

### Was als Nächstes kommt

1. **M1 Login:** JWT-Chain, `ServerToClientHandshake`, AES, Compression,
   `StartGame` + Chunk-Streaming → Vanilla-Client kann joinen.
2. **M2 Welt:** LevelDB lesen/schreiben, Block-States, Licht, Nachbar-Updates.
3. **M3 Gameplay:** Entities, Physik, Schaden/Tod, Inventar-Transaktionen.
4. **M4 API-Parität:** alle PMMP-Events, Form-API, Command-Parser, Selektoren.
5. **M5 Betrieb:** Timings, Crashdumps, Resource-Packs, Query/RCON, CI.

Details, Namespace-Mapping und Abnahmekriterien: [`PORTING.md`](PORTING.md).

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

Requirements: Rust 1.75+ (tested with 1.100) and the **.NET 10 SDK**.

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
| `/world <info\|list\|time\|settime\|getblock\|setblock\|tp>` | world and block access |
| `/event <join\|quit\|chat\|break\|place\|damage\|death\|respawn>` | dispatch a test event to plugins |
| `/reload` | disable, reload and re-enable plugins |
| `/stop` | save worlds and shut down |
| `/csharp`, `/hello`, `/build` | provided by `ExamplePlugin` |

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
