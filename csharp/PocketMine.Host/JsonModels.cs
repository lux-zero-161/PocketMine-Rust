using System.Text.Json;
using PocketMine.Item;
using PocketMine.Math;
using PocketMine.Player;
using PocketMine.Server;
using PocketMine.World;

namespace PocketMine.Host;

/// <summary>Converts the Rust JSON snapshots into SDK model objects.</summary>
public static class JsonModels
{
    public static string Str(JsonElement element, string property, string fallback = "")
        => element.ValueKind == JsonValueKind.Object && element.TryGetProperty(property, out JsonElement value) && value.ValueKind == JsonValueKind.String
            ? value.GetString() ?? fallback
            : fallback;

    public static double Num(JsonElement element, string property, double fallback = 0)
        => element.ValueKind == JsonValueKind.Object && element.TryGetProperty(property, out JsonElement value) && value.ValueKind == JsonValueKind.Number
            ? value.GetDouble()
            : fallback;

    public static int Int(JsonElement element, string property, int fallback = 0)
        => (int)Num(element, property, fallback);

    public static bool Bool(JsonElement element, string property, bool fallback = false)
        => element.ValueKind == JsonValueKind.Object && element.TryGetProperty(property, out JsonElement value) &&
           (value.ValueKind == JsonValueKind.True || value.ValueKind == JsonValueKind.False)
            ? value.GetBoolean()
            : fallback;

    public static Player.Player Player(JsonElement element, IServer? server = null)
    {
        var player = new Player.Player
        {
            Name = Str(element, "name", "Player"),
            DisplayName = Str(element, "display_name", Str(element, "name", "Player")),
            Uuid = Str(element, "uuid"),
            Xuid = Str(element, "xuid"),
            EntityId = (ulong)Num(element, "entity_id"),
            Address = Str(element, "address"),
            Port = Int(element, "port"),
            WorldName = Str(element, "world"),
            Position = new Vector3(Num(element, "x"), Num(element, "y"), Num(element, "z")),
            Yaw = (float)Num(element, "yaw"),
            Pitch = (float)Num(element, "pitch"),
            Health = (float)Num(element, "health", 20),
            MaxHealth = (float)Num(element, "max_health", 20),
            Food = Int(element, "food", 20),
            XpLevel = Int(element, "xp_level"),
            GameMode = (GameMode)Int(element, "gamemode"),
            IsOp = Bool(element, "is_op"),
            IsOnline = Bool(element, "online", true),
            Server = server!,
        };
        string held = Str(element, "held_item");
        if (!string.IsNullOrEmpty(held) && held != "minecraft:air")
        {
            player.Inventory.SetItem(0, ItemStack.Of(held, 1));
        }
        return player;
    }

    public static Block BlockFromJson(JsonElement element) => new(
        Str(element, "name", "minecraft:air"),
        Int(element, "id"),
        (ushort)Int(element, "meta"),
        Num(element, "hardness"),
        Num(element, "blast_resistance"),
        Int(element, "light"),
        Bool(element, "solid") && !Bool(element, "air")
    );

    public static World.World World(JsonElement element)
    {
        string name = Str(element, "name");
        long time = (long)Num(element, "time");
        Vector3 spawn = element.TryGetProperty("spawn", out JsonElement spawnElement)
            ? new Vector3(Num(spawnElement, "x"), Num(spawnElement, "y"), Num(spawnElement, "z"))
            : Vector3.Zero;

        return new World.World(
            name,
            time,
            spawn,
            (x, y, z) =>
            {
                string? json = Native.GetBlock(name, x, y, z);
                if (json == null) return Block.Air;
                using var document = JsonDocument.Parse(json);
                return BlockFromJson(document.RootElement);
            },
            (x, y, z, block, meta) => Native.SetBlock(name, x, y, z, block, meta)
        )
        {
            LoadedChunks = Int(element, "chunks"),
            EntityCount = Int(element, "entities"),
            Generator = Str(element, "generator", "flat"),
            Difficulty = Int(element, "difficulty"),
        };
    }

    public static ServerInfo ServerInfo(string json)
    {
        using var document = JsonDocument.Parse(json);
        JsonElement root = document.RootElement;
        return new ServerInfo
        {
            Name = Str(root, "name", "PocketMine-Rust"),
            Version = Str(root, "version"),
            ApiVersion = Str(root, "api_version"),
            MinecraftVersion = Str(root, "minecraft_version"),
            Protocol = Int(root, "protocol"),
            Tick = (ulong)Num(root, "tick"),
            Tps = Num(root, "tps", 20),
            Online = Int(root, "online"),
            MaxPlayers = Int(root, "max_players"),
            Uptime = (long)Num(root, "uptime"),
            Motd = Str(root, "motd"),
            Worlds = StringArray(root, "worlds"),
            Plugins = StringArray(root, "plugins"),
        };
    }

    public static IReadOnlyList<string> StringArray(JsonElement root, string property)
    {
        if (!root.TryGetProperty(property, out JsonElement array) || array.ValueKind != JsonValueKind.Array)
        {
            return Array.Empty<string>();
        }
        return array.EnumerateArray()
            .Where(e => e.ValueKind == JsonValueKind.String)
            .Select(e => e.GetString() ?? "")
            .ToArray();
    }
}
