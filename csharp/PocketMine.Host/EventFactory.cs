using System.Text.Json;
using PocketMine.Command;
using PocketMine.Event;
using PocketMine.Math;
using PocketMine.Player;

namespace PocketMine.Host;

/// <summary>Turns the JSON payload emitted by the Rust event structs into typed SDK events.</summary>
public static class EventFactory
{
    public static Event.Event? Create(int eventType, JsonElement root, PluginHostServer server)
    {
        switch (eventType)
        {
            case 1:
                return new PlayerJoinEvent(PlayerFor(root, server), JsonModels.Str(root, "join_message"));
            case 2:
                return new PlayerQuitEvent(PlayerFor(root, server), JsonModels.Str(root, "quit_message"));
            case 3:
                return new PlayerChatEvent(
                    PlayerFor(root, server),
                    JsonModels.Str(root, "message"),
                    JsonModels.Str(root, "format", "<{0}> {1}")
                );
            case 4:
                return new BlockBreakEvent(
                    PlayerFor(root, server),
                    Position(root, "block_pos"),
                    (uint)JsonModels.Int(root, "block_id"),
                    JsonModels.Str(root, "block_name")
                );
            case 5:
                return new BlockPlaceEvent(
                    PlayerFor(root, server),
                    Position(root, "block_pos"),
                    (uint)JsonModels.Int(root, "block_id"),
                    JsonModels.Str(root, "block_name")
                );
            case 6:
                return new ServerCommandEvent(SenderFor(JsonModels.Str(root, "sender_name", "CONSOLE"), server), JsonModels.Str(root, "command"));
            case 7:
                return new PlayerCommandPreprocessEvent(PlayerFor(root, server), JsonModels.Str(root, "message"));
            case 8:
                return new PlayerMoveEvent(
                    PlayerFor(root, server),
                    new Vector3(JsonModels.Num(root, "from_x"), JsonModels.Num(root, "from_y"), JsonModels.Num(root, "from_z")),
                    new Vector3(JsonModels.Num(root, "to_x"), JsonModels.Num(root, "to_y"), JsonModels.Num(root, "to_z"))
                );
            case 9:
                return new PlayerInteractEvent(
                    PlayerFor(root, server),
                    new BlockPosition(JsonModels.Int(root, "block_x"), JsonModels.Int(root, "block_y"), JsonModels.Int(root, "block_z")),
                    JsonModels.Str(root, "block_name", "minecraft:air"),
                    JsonModels.Str(root, "action", "right-click")
                );
            case 10:
                return new EntityDamageEvent(
                    JsonModels.Str(root, "entity_name", "Entity"),
                    JsonModels.Num(root, "damage"),
                    JsonModels.Str(root, "cause", "generic")
                );
            case 11:
                return new PlayerDeathEvent(PlayerFor(root, server), JsonModels.Str(root, "death_message"));
            case 12:
                return new PlayerRespawnEvent(
                    PlayerFor(root, server),
                    new Vector3(JsonModels.Num(root, "x"), JsonModels.Num(root, "y"), JsonModels.Num(root, "z"))
                );
            case 13:
                return new ServerStartEvent();
            case 14:
                return new ServerStopEvent();
            case 15:
            {
                string name = JsonModels.Str(root, "world_name");
                return new WorldLoadEvent(server.GetWorld(name) ?? new World.World { Name = name });
            }
            case 16:
            {
                string name = JsonModels.Str(root, "world_name");
                return new WorldUnloadEvent(server.GetWorld(name) ?? new World.World { Name = name });
            }
            default:
                return null;
        }
    }

    private static BlockPosition Position(JsonElement root, string property)
    {
        if (root.ValueKind == JsonValueKind.Object && root.TryGetProperty(property, out JsonElement element))
        {
            return new BlockPosition(
                JsonModels.Int(element, "x"),
                JsonModels.Int(element, "y"),
                JsonModels.Int(element, "z")
            );
        }
        return BlockPosition.Zero;
    }

    private static ICommandSender SenderFor(string name, PluginHostServer server)
    {
        if (name.Equals("CONSOLE", StringComparison.OrdinalIgnoreCase))
        {
            return server.ConsoleSender;
        }
        return server.GetPlayer(name) ?? server.ConsoleSender;
    }

    private static Player.Player PlayerFor(JsonElement root, PluginHostServer server)
    {
        string name = JsonModels.Str(root, "player_name", "Player");
        return server.GetPlayer(name) ?? new Player.Player
        {
            Name = name,
            DisplayName = name,
            Uuid = JsonModels.Str(root, "player_uuid"),
            WorldName = JsonModels.Str(root, "world_name"),
            Position = new Vector3(JsonModels.Num(root, "x"), JsonModels.Num(root, "y"), JsonModels.Num(root, "z")),
            Health = 20f,
            MaxHealth = 20f,
            IsOnline = true,
            Server = server,
        };
    }
}
