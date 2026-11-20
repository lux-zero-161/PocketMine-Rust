namespace PocketMine.Item;

public class ItemStack : IEquatable<ItemStack>
{
    public string Name { get; set; }
    public int Id { get; set; }
    public ushort Meta { get; set; }
    public byte Count { get; set; }
    public string? CustomName { get; set; }

    public static readonly ItemStack Air = new("minecraft:air", 0, 0, 0);

    public ItemStack(string name, int id = 0, ushort meta = 0, byte count = 1, string? customName = null)
    {
        Name = name;
        Id = id;
        Meta = meta;
        Count = count;
        CustomName = customName;
    }

    public static ItemStack Of(string name, byte count = 1) => new(name, 0, 0, count);

    public bool IsEmpty => Count == 0 || Name == "minecraft:air";

    public ItemStack Clone() => new(Name, Id, Meta, Count, CustomName);

    public bool Equals(ItemStack? other)
        => other != null && Id == other.Id && Meta == other.Meta && Name == other.Name;

    public override bool Equals(object? obj) => obj is ItemStack other && Equals(other);
    public override int GetHashCode() => HashCode.Combine(Name, Id, Meta);
    public override string ToString() => $"ItemStack({Name} x{Count})";
}
