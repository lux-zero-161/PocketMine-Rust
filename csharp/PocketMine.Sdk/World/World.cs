using PocketMine.Math;

namespace PocketMine.World;

public sealed class Block
{
    public string Name { get; }
    public int Id { get; }
    public ushort Meta { get; }
    public double Hardness { get; }
    public double BlastResistance { get; }
    public int LightLevel { get; }
    public bool IsSolid { get; }

    public static readonly Block Air = new("minecraft:air", 0, 0, 0, 0, 0, false);

    public Block(string name, int id = 0, ushort meta = 0, double hardness = 0, double blastResistance = 0, int lightLevel = 0, bool isSolid = true)
    {
        Name = name;
        Id = id;
        Meta = meta;
        Hardness = hardness;
        BlastResistance = blastResistance;
        LightLevel = lightLevel;
        IsSolid = isSolid;
    }

    public bool IsAir => Name == "minecraft:air";
    public override string ToString() => $"Block({Name})";
}

public sealed class World
{
    private readonly Func<int, int, int, Block>? _getBlock;
    private readonly Func<int, int, int, string, ushort, bool>? _setBlock;

    public string Name { get; internal set; } = string.Empty;
    public long Time { get; internal set; }
    public Vector3 Spawn { get; internal set; } = Vector3.Zero;
    public int LoadedChunks { get; internal set; }
    public int EntityCount { get; internal set; }
    public string Generator { get; internal set; } = "flat";
    public int Difficulty { get; internal set; }

    public World() { }

    public World(string name, long time, Vector3 spawn, Func<int, int, int, Block>? getBlock, Func<int, int, int, string, ushort, bool>? setBlock)
    {
        Name = name;
        Time = time;
        Spawn = spawn;
        _getBlock = getBlock;
        _setBlock = setBlock;
    }

    public Block GetBlock(int x, int y, int z) => _getBlock?.Invoke(x, y, z) ?? Block.Air;

    public Block GetBlock(Vector3 position) => GetBlock((int)System.Math.Floor(position.X), (int)System.Math.Floor(position.Y), (int)System.Math.Floor(position.Z));

    public bool SetBlock(int x, int y, int z, Block block) => _setBlock?.Invoke(x, y, z, block.Name, block.Meta) ?? false;

    public bool SetBlock(Vector3 position, Block block) => SetBlock((int)System.Math.Floor(position.X), (int)System.Math.Floor(position.Y), (int)System.Math.Floor(position.Z), block);

    public bool SetBlock(int x, int y, int z, string blockName, ushort meta = 0) => _setBlock?.Invoke(x, y, z, blockName, meta) ?? false;

    public override string ToString() => $"World({Name})";
}
