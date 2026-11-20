# Portierungsplan – PocketMine-MP (PHP) → PocketMine-Rust

Ziel: **verhaltensgleicher 1:1-Port** von PocketMine-MP (Basis `5.51.2`, Minecraft
Bedrock `1.26.60`, Protokoll `2225`) auf einen **Rust-Core** mit **C#-Plugins**.

Die Vorlage ist `../PocketMine-MP` mit **1.797 PHP-Dateien / 168.434 Zeilen**.

> Ehrliche Einordnung: 168k Zeilen PHP lassen sich nicht in einem Durchgang
> portieren. Dieses Repository ist ab jetzt die lauffähige Basis, auf der die
> Portierung Namespace für Namespace weiterläuft. Was bereits echt funktioniert,
> steht unter „Aktueller Meilenstein“ – nichts davon ist ein Stub.

---

## Aktueller Meilenstein (verifiziert)

| Bereich | Status | Nachweis |
|---|---|---|
| Rust-Workspace mit 11 Crates | ✅ | `cargo build`, `cargo test --workspace` |
| 20-TPS-Tick-Loop, Console, Logdatei | ✅ | `logs/server.log`, `/tps` |
| Task-Scheduler (Rust + C#) | ✅ | wiederholende Tasks laufen über 130 Ticks |
| 1.26.60-Protokollkonstanten | ✅ | `ProtocolInfo` = 2225 |
| Block-/Item-Registry aus `bedrock-data` | ✅ | 1.000+ Items, 900+ Blöcke, echte Härtewerte |
| Welt: Chunks, Paletten-Subchunks, Flat-Generator | ✅ | `/world getblock`, `/world setblock` |
| Spieler, Inventar (36+4+1), Gamemodes, Rechte | ✅ | `pm_player`, C# `Player` |
| C#-Plugin-Host in-process (CoreCLR/hostfxr) | ✅ | `ExamplePlugin` lädt, aktiviert, deaktiviert |
| C#-Events mit Priorität, Cancel und **Patch-Back** | ✅ | Join-Text aus C# erscheint im Server |
| C#-Commands mit Aliassen/Permissions | ✅ | `/csharp`, `/hello`, `/build` |
| C#-Scheduler + Config + World-/Block-API | ✅ | `Server.Scheduler`, `world.SetBlock` |
| RakNet: Unconnected Ping/Pong (LAN-Liste) | ✅ | `tools/ping.py` |
| RakNet: Offline-Handshake (Request 1/2) | ✅ | Reply 1/2 mit MTU/GUID |
| RakNet: ACKs, Split-Frames, Reassembly | ✅ | Tests in `pm_network` |
| Bedrock-Batches + Raw-Deflate-Kompression | ✅ | Threshold 256, Typ-Präfix 1.20.60+ |
| Bedrock `RequestNetworkSettings` → `NetworkSettings` | ✅ | Ende-zu-Ende-Netzwerktest |
| Offline-`Login` parsen (Identität aus JWT-Chain) | ✅ | Name/UUID/XUID + Client-Public-Key |
| ECDH P-384 + ES384 Handshake-JWT | ✅ | `ServerToClientHandshake`, Signatur verifiziert |
| AES-256-CFB8 + PMMP-Checksummen | ✅ | Roundtrip- und Tamper-Tests |
| Verschlüsselter `ClientToServerHandshake` → `LoginSuccess` | ✅ | Ende-zu-Ende-Test inkl. Entschlüsselung |
| `ResourcePacksInfo` / `ResourcePackStack` + Client-Responses | ✅ | Status 2/3, Stack-Antwort |
| `RequestChunkRadius` → `ChunkRadiusUpdated`, `SetTime` | ✅ | Radius wird begrenzt |
| Login → echter Spieler + Join/Quit-Events | ✅ | `process_network_events`, Session-Timeout |
| `StartGame` + LevelSettings + Block-Palette | ✅ | experimentell, Encoder + Tests |
| Sende-Pakete: Jigsaw, VoxelShapes, ItemRegistry, BiomeDefs | ✅ | minimal/leer, aber korrekt gerahmt |
| `LevelChunk`-Streaming mit Paletten-Subchunks | ✅ | eigener Encoder, Chunk-Burst |
| NBT-Writer/Reader (Bedrock-Netzwerkformat) | ✅ | `pm_nbt`, Palette + Chunkdaten |
| UDP-Integrationstest Login → StartGame → Chunk | ✅ | echter Socket, Split-Reassembly |
| Spawn-Sequenz (`AvailableCommands`, `UpdateAbilities`, `MovePlayer`, `PLAYER_SPAWN`) | ✅ | Client kann Loading-Screen verlassen |
| Client-Chat (`Text`) + `PlayerChatEvent` | ✅ | Broadcast zurück an alle Clients |
| Client-Commands (`CommandRequest`) + Preprocess-Event | ✅ | Ausführung über die Command-Map |
| Server-/Plugin-Nachrichten als `Text`-Pakete | ✅ | `flush_player_messages` sendet echt |
| Block-Abbau (`PlayerAction`) + `UpdateBlock`-Broadcast | ✅ | Creative/Survival-Abbau, Event |
| Block-Platzierung (`InventoryTransaction` UseItemOn) | ✅ | Item aus dem Paket, Event, Broadcast |
| Bewegung (`PlayerAuthInput`) + `PlayerMoveEvent` | ✅ | Position/Rotation, Broadcast an andere Spieler |
| Block-Actions innerhalb `PlayerAuthInput` | ✅ | Bewegung + Block-Aktionen aus einem Paket |
| Welt-Persistenz (`world.json` + `chunks.bin`) | ✅ | Zwischenformat, Autosave, `/save` |
| Bedrock-LevelDB-Weltformat | ⏳ | langfristiges Ziel (M2) |
| Echter Vanilla-Client-Test | ⏳ | unmittelbarer nächster Schritt |
| Weltformate (LevelDB/Anvil), NBT, Entities-KI | ⏳ | nächste Meilensteine |

---

## Namespace-Mapping

Zeilenzahlen aus `../PocketMine-MP/src`. Status: ✅ vorhanden, 🟡 teilweise,
⏳ offen.

| PMMP `src/` | Dateien | Zeilen | Rust-Ziel | Status |
|---|---:|---:|---|---|
| `block/` | 494 | 41.092 | `pm_world::Block`, `pm_data` | 🟡 Registry + Platzieren/Abbauen |
| `world/` | 299 | 24.606 | `pm_world`, `pm_scheduler` | 🟡 Chunks/Zeit/Generator, kein Save-Format |
| `network/` | 111 | 16.237 | `pm_network` | 🟡 RakNet, Batches, Kompression; Login-Verschlüsselung offen |
| `entity/` | 178 | 14.937 | `pm_world::Entity` | ⏳ nur Basismodell |
| `item/` | 174 | 14.460 | `pm_player::ItemStack`, `pm_data` | 🟡 Stacks + Registry |
| `data/` | 101 | 13.210 | `pm_data`, `pm_nbt` | 🟡 bedrock-data + Netzwerk-NBT, keine Upgrader |
| `event/` | 155 | 9.424 | `pm_event` + C# `PocketMine.Event` | 🟡 16 Kern-Events |
| `command/` | 57 | 5.254 | `pm_command` + C# `Command` | 🟡 Map/Aliasse, kein Vanilla-Parser |
| `utils/` | 35 | 5.190 | `pm_core` (Logger, Config, Text) | 🟡 Kern-Utilities |
| `player/` | 16 | 3.894 | `pm_player` | 🟡 Session/Inventar, keine XP-Logik |
| `inventory/` | 39 | 3.506 | `pm_player::PlayerInventory` | 🟡 Container fehlen |
| `plugin/` | 22 | 2.521 | `pm_plugin_host` + C# SDK | 🟡 Lifecycle/Events/Commands |
| `crafting/` | 27 | 2.192 | – | ⏳ |
| `scheduler/` | 15 | 1.624 | `pm_scheduler` | ✅ Tick-Tasks |
| `permission/` | 14 | 1.566 | `pm_permission` | 🟡 Registry + Attachments |
| `timings/` | 3 | 936 | – | ⏳ |
| `thread/` | 13 | 930 | Rust-Threads | 🟡 Console/Netz getrennt |
| `resourcepacks/` | 9 | 730 | – | ⏳ |
| `crash/` | 5 | 591 | – | ⏳ |
| `console/` | 5 | 475 | `pm_command` | 🟡 Console-Sender |
| `lang/` | 3 | 380 | `pm_core` | ⏳ |
| `updater/` | 3 | 308 | – | ⏳ |
| `form/` | 3 | 132 | – | ⏳ |
| `promise/` | 3 | 234 | – | Rust-`async` geplant |

---

## Rust-Crate-Layout

| Crate | Aufgabe | Zeilen |
|---|---|---:|
| `pm_core` | Konstanten, Logger, `server.properties`, `pocketmine.yml`, Textfarben | ~600 |
| `pm_math` | Vector3/Vector2, BlockPosition, Facing, AABB, Location, RNG | ~500 |
| `pm_data` | Block-/Item-Registry aus `bedrock-data` (inkl. Datenkopie) | ~200 |
| `pm_event` | 16 Events, Prioritäten, Cancel, JSON-Patch | ~300 |
| `pm_permission` | Permissions, Defaults, Attachments | ~180 |
| `pm_command` | Command-Map, Aliase, Sender, Argument-Splitting | ~300 |
| `pm_scheduler` | verzögerte/wiederholende Tick-Tasks | ~180 |
| `pm_world` | Chunks, Subchunk-Paletten, Block-API, Weltmanager, Entities | ~700 |
| `pm_player` | Spieler, Gamemodes, Inventar, Spielermanager | ~450 |
| `pm_network` | RakNet-Ping/Handshake, Frame-Codec, NetworkSettings | ~750 |
| `pm_plugin_host` | hostfxr, C-ABI, Command-Proxy, Event-Bridge | ~700 |
| `pm_server` | Bootstrap, Tick-Loop, Commands, ServerBridge | ~1.300 |

---

## C#-Plugin-ABI

Rust ruft Managed Code über `[UnmanagedCallersOnly]` auf; es gibt **keinen
Socket und keine IPC**. Die ABI-Tabelle liegt in
`crates/pm_plugin_host/src/lib.rs` (`RustApiTable`) und
`csharp/PocketMine.Host/NativeApi.cs` – Feldreihenfolge ist bindend.

| Export | Richtung | Zweck |
|---|---|---|
| `InitializeHost(table)` | Rust → C# | übergibt die Callback-Tabelle |
| `LoadPlugins(dir)` | Rust → C# | scannt Ordner/Unterordner nach Plugins |
| `EnablePlugins` / `DisablePlugins` | Rust → C# | Lifecycle |
| `DispatchEvent(type,json,cancel,out,cap)` | Rust → C# | Event + Ergebnis-Patch |
| `ExecuteCommand(...)` | Rust → C# | Plugin-Command ausführen |
| `TickScheduler(tick)` | Rust → C# | C#-Tasks im Server-Takt |
| `ListPlugins()` | Rust → C# | `/plugins`, `/reload` |
| `log`, `broadcast`, `send_message`, `kick`, `teleport` | C# → Rust | Spieler/Server-Aktionen |
| `register_command`, `unregister_command`, `dispatch_command` | C# → Rust | Command-System |
| `get_player`, `get_players`, `get_worlds`, `get_block`, `set_block` | C# → Rust | Welt-/Spielerabfragen |
| `get_server_info`, `register_permission`, `shutdown` | C# → Rust | Server-Metadaten |

**Event-Patch:** C# liefert nur geänderte Felder zurück
(`join_message`, `message`, `format`, `damage`, `death_message`,
`command`, `to_x/y/z`, `x/y/z`). Rust merged sie in sein Event und arbeitet
danach mit den geänderten Werten weiter – so wie PMMP es erlaubt.

**Strings:** Rust → C# gepinnte Rust-Strings werden über `free_string`
freigegeben. C# → Rust (`ListPlugins`) nutzt pro Prozess gepinnte Puffer, die
Rust nur liest.

---

## Nächste Meilensteine (Reihenfolge)

### M1 – Login & Netzwerk (kritisch für echte Clients)
1. ✅ `Login`-Paket parsen, Offline-Identität, Client-Public-Key.
2. ✅ `ServerToClientHandshake` (ECDH P-384, ES384), AES-256-CFB8, Checksummen.
3. ✅ Compression (raw Deflate), Batches, Split-Frames.
4. ✅ `ResourcePacksInfo`, `ResourcePackStack`, Client-Responses.
5. ✅ `RequestChunkRadius`, `ChunkRadiusUpdated`, `SetTime`, Session-Timeout.
6. ✅ `StartGame` + Block-Palette, Item-Registry, Jigsaw/VoxelShapes/BiomeDefs.
7. ✅ Chunk-Streaming (`LevelChunk` mit Paletten-Encoding).
8. ⏳ Gegen einen echten 1.26.60-Client verifizieren (Biome-Definitionen,
   Item-Component-NBT, AvailableCommands).
   **Abnahme:** Vanilla-Client 1.26.60 joint, sieht flache Welt, kann laufen.

### M2 – Welt & Blöcke
1. `LevelDB`-Format (Bedrock) lesen/schreiben; `world.json` nur Übergang.
2. Block-State-Properties (`block_properties_table.json` + `canonical_block_states`).
3. Nachbar-Updates, Licht, Wasser/Lava-Fluss.
   **Abnahme:** Bauen/Abbauen wie in PMMP, Welt bleibt nach Restart erhalten.

### M3 – Entities & Gameplay
1. Entity-Typ-Registry, Metadaten, Bewegung/Physik.
2. Schaden, Tod, Respawn, Drops, XP.
3. Inventar-Transaktionen, Container, Crafting-Regeln.
   **Abnahme:** Zombie spawnt und greift an; Inventar bleibt konsistent.

### M4 – Plugin-API-Parität
1. Alle PMMP-Events (155 Dateien) als C#-Klassen + JSON-Mapping.
2. `Form`-API (SimpleForm/CustomForm/ModalForm) mit Bedrock-Form-Paketen.
3. Command-Argument-Parser inkl. Selektoren (`@p`, `@a`, `@r`, `@s`).
4. Permissions-Defaults, `plugin.yml`-äquivalentes `plugin.json`.
   **Abnahme:** Ein bestehendes PMMP-Plugin lässt sich mit mechanischem
   Umbenennen nach C# portieren.

### M5 – Betrieb
1. `timings`, Crashdumps, `--generate`, `--version`, Auto-Updater.
2. Resource-Packs, Query, RCON, IPv6.
3. Docker-Image, CI (cargo test + dotnet build + Smoke-Test).

---

## Arbeitsweise pro PHP-Namespace

1. **Typen zuerst:** PHP-Klassen → Rust-Structs/Enums, 1:1 Feldnamen.
2. **Verhalten:** Methode für Methode portieren; PHP-Sonderfälle (Int-Division,
   `null`, Referenzen) explizit abbilden.
3. **Tests:** Für jede portierte Einheit einen Rust-Unit-Test mit Werten aus
   PMMP (z. B. Härtewerte, Distanzen, Tick-Berechnungen).
4. **Events:** Rust-Event-Struct + C#-Klasse + `EventFactory`-Case +
   `EventPatcher`-Case, dann im `ExamplePlugin` nutzen.
5. **Keine Fantasie-APIs:** C#-SDK spiegelt PMMP-Namen (`PlayerJoinEvent`,
   `getServer`, `broadcastMessage`, `TaskScheduler`), damit Ports mechanisch
   bleiben.

---

## Verifikation

```bash
./build.sh                 # Rust + SDK + Host + Beispielplugin
cargo test --workspace     # Unit-Tests (Registry, Welt, Netz, Inventar)
./tools/smoke-test.sh      # kompletter End-to-End-Test inkl. C#-Plugin
python3 tools/ping.py 19132 # LAN-Ping gegen einen laufenden Server
```

Erwartete Smoke-Test-Ausgabe (Auszug):

```
Loaded C# plugin ExamplePlugin v1.0.0 by Zero [API 1.0.0.0]
Alex joined the game (via C# event)
[Alex] Welcome to PocketMine-Rust, Alex!
Server stopped after 130 tick(s).
```
