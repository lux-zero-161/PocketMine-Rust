namespace PocketMine.Item;

public class ItemStack
{
    public uint Id { get; set; }
    public ushort Meta { get; set; }
    public byte Count { get; set; }
    public string? CustomName { get; set; }

    public static readonly ItemStack Air = new(0, 0, 0);

    public ItemStack(uint id, ushort meta = 0, byte count = 1, string? customName = null)
    {
        Id = id;
        Meta = meta;
        Count = count;
        CustomName = customName;
    }

    public bool IsEmpty => Id == 0 || Count == 0;

    public override string ToString() => $"ItemStack(Id={Id}, Meta={Meta}, Count={Count}, Name={CustomName ?? "None"})";
}
