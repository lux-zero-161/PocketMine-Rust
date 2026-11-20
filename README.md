# PocketMine-Rust

A high-performance Minecraft: Bedrock Edition server software written in **Rust** with native in-process **C# (.NET 10)** plugin support.

PocketMine-Rust ports the architecture of [PocketMine-MP](https://github.com/pmmp/PocketMine-MP) to modern Rust for maximum concurrency, memory safety, and low latency, while providing a powerful C# Plugin API for developers.

---

## 🌟 Key Features

- **Blazing Fast Server Core (Rust)**:
  - 20 TPS asynchronous server tick loop with sub-millisecond precision.
  - Multi-threaded console input, packet dispatcher, and world ticking.
  - UDP RakNet network engine implementing Minecraft Bedrock Edition protocol (v685 / v1.21+).
  - Modern Chunk / SubChunk voxel world representation with flat generation and block ticking.
  - Full player session, inventory (36 slots + 4 armor + offhand), permissions, and command system.
- **Native C# (.NET 10) Plugin System**:
  - Plugins are written in **100% C#** (.NET 10 class libraries).
  - Hosted directly inside the Rust server process via official Microsoft `.NET CoreCLR` (`hostfxr`).
  - Zero socket / IPC overhead: ultra-fast in-process C-ABI / FFI interop.
  - Supports modern C# features: LINQ, async/await, reflection, NuGet dependencies, etc.
  - Event bus with `@EventHandler` priority system (`Lowest` to `Monitor`) and event cancellation.
  - Dynamic command registration directly from C# into the Rust server command map.
- **Cross-Platform**:
  - Native Linux binary (`pocketmine` / `PocketMine` / `./start.sh`)
  - Native Windows binary (`PocketMine.exe` / `start.cmd` / `start.ps1`)

---

## 📁 Repository Structure

```text
PocketMine-Rust/
├── Cargo.toml                  # Rust workspace configuration
├── .gitignore                  # Git ignore rules for Rust, .NET, and server data
├── server.properties           # Bedrock server network & gameplay configuration
├── pocketmine.yml              # Engine tuning and plugin settings
├── build.sh                    # Linux build script
├── build.cmd                   # Windows build script
├── start.sh                    # Linux server launcher
├── start.cmd                   # Windows CMD server launcher
├── start.ps1                   # Windows PowerShell launcher
│
├── crates/                     # Rust Server Crates
│   ├── pm_core/                # Logger, Server Constants, Configs (server.properties)
│   ├── pm_math/                # Vector3, BlockPosition, Facing, AxisAlignedBB
│   ├── pm_event/               # Event types, Cancellable trait, EventPriority
│   ├── pm_command/             # CommandMap, Command trait, Default commands (/help, /stop, etc.)
│   ├── pm_world/               # World, Chunk, SubChunk, Block registry
│   ├── pm_player/              # Player model, GameMode, Inventory & ItemStack
│   ├── pm_network/             # Bedrock RakNet UDP listener & packet handler
│   ├── pm_plugin_host/         # HostFXR CoreCLR runtime loader & FFI bridge
│   └── pm_server/              # Main executable (pmmp / PocketMine) & tick loop
│
├── csharp/                     # C# .NET Ecosystem
│   ├── PocketMine.Sdk/         # C# API library referenced by plugin authors
│   ├── PocketMine.Host/        # In-process .NET host assembly invoked by Rust
│   └── plugins/
│       └── ExamplePlugin/      # Demonstration C# plugin (Commands, Events)
│
└── plugins/                    # Directory for compiled C# plugin .dll files
```

---

## 🚀 Building & Running

### Prerequisites

1. **Rust Toolchain**: `rustc` and `cargo` (1.75+ or 1.100+)
2. **.NET SDK**: .NET 8, 9, or 10 (`dotnet` CLI)

### On Linux

```bash
# 1. Build the entire ecosystem (SDK, Host, Plugins, and Server):
./build.sh

# 2. Start the server:
./start.sh
```

### On Windows

```cmd
:: 1. Build the ecosystem:
build.cmd

:: 2. Start the server:
start.cmd
```
*(Or in PowerShell: `.\start.ps1`)*

---

## 💻 Writing a C# Plugin

Creating a plugin in PocketMine-Rust is simple and familiar to anyone who has written plugins for PocketMine-MP or Spigot.

### 1. Create a C# Project

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <ImplicitUsings>enable</ImplicitUsings>
    <Nullable>enable</Nullable>
  </PropertyGroup>

  <ItemGroup>
    <ProjectReference Include="..\..\csharp\PocketMine.Sdk\PocketMine.Sdk.csproj" />
  </ItemGroup>
</Project>
```

### 2. Implement `PluginBase`

```csharp
using PocketMine.Plugin;
using PocketMine.Command;
using PocketMine.Event;

namespace MyAwesomePlugin;

[Plugin("MyAwesomePlugin", "1.0.0", Author = "YourName", Description = "A C# Bedrock Plugin")]
public class Main : PluginBase
{
    public override void OnLoad()
    {
        Logger.Info("MyAwesomePlugin is loading!");
    }

    public override void OnEnable()
    {
        Logger.Info("MyAwesomePlugin enabled!");

        // Register custom commands
        Server.RegisterCommand(new HelloCommand());

        // Register event listeners
        Server.RegisterEvents(new MyListener(this), this);
    }

    public override void OnDisable()
    {
        Logger.Info("MyAwesomePlugin disabled!");
    }
}
```

### 3. Handle Events

```csharp
public class MyListener : IListener
{
    private readonly Main _plugin;

    public MyListener(Main plugin) => _plugin = plugin;

    [EventHandler(Priority = EventPriority.Normal)]
    public void OnPlayerJoin(PlayerJoinEvent ev)
    {
        ev.Player.SendMessage($"§aWelcome §e{ev.Player.Name}§a to our PocketMine-Rust server!");
    }

    [EventHandler(Priority = EventPriority.High)]
    public void OnPlayerChat(PlayerChatEvent ev)
    {
        if (ev.Message.StartsWith("!ping"))
        {
            ev.Player.SendMessage("§bPong from C#!");
            ev.IsCancelled = true; // Cancels the chat message
        }
    }
}
```

### 4. Create Commands

```csharp
public class HelloCommand : Command
{
    public HelloCommand() : base("hello", "Say hello", "/hello") {}

    public override bool Execute(ICommandSender sender, string label, string[] args)
    {
        sender.SendMessage("§6Hello from C# running inside Rust!");
        return true;
    }
}
```

### 5. Deploy

Compile the plugin and copy the generated `.dll` into the `plugins/` directory:

```bash
dotnet build -c Release
cp bin/Release/net10.0/MyAwesomePlugin.dll ../../plugins/
```

Restart or start the server; your plugin will be loaded automatically!

---

## 📜 Default Commands

- `/help` — List all registered commands (both Rust native and C# plugin commands).
- `/version` — Show PocketMine-Rust engine and .NET runtime details.
- `/plugins` — List all loaded C# plugins.
- `/csharp` — Test command provided by `ExamplePlugin`.
- `/say <msg>` — Broadcast a message to the console and online players.
- `/list` — Display connected players.
- `/stop` — Gracefully stop the server, saving worlds and disabling plugins.

---

## 📄 License

Licensed under the [LGPL-3.0 License](LICENSE) (matching PocketMine-MP).
