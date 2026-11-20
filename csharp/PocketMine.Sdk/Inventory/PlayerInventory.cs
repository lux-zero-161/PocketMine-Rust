using PocketMine.Item;

namespace PocketMine.Inventory;

public class PlayerInventory
{
    private readonly ItemStack[] _slots = new ItemStack[36];
    private readonly ItemStack[] _armor = new ItemStack[4];
    public ItemStack Offhand { get; set; } = ItemStack.Air;
    public int HeldSlot { get; set; } = 0;

    public PlayerInventory()
    {
        for (int i = 0; i < _slots.Length; i++) _slots[i] = ItemStack.Air;
        for (int i = 0; i < _armor.Length; i++) _armor[i] = ItemStack.Air;
    }

    public ItemStack GetItem(int slot) => slot >= 0 && slot < _slots.Length ? _slots[slot] : ItemStack.Air;

    public void SetItem(int slot, ItemStack item)
    {
        if (slot >= 0 && slot < _slots.Length) _slots[slot] = item;
    }

    public ItemStack GetHeldItem() => GetItem(HeldSlot);

    public bool AddItem(ItemStack item)
    {
        for (int i = 0; i < _slots.Length; i++)
        {
            if (_slots[i].IsEmpty)
            {
                _slots[i] = item;
                return true;
            }
        }
        return false;
    }

    public void Clear()
    {
        for (int i = 0; i < _slots.Length; i++) _slots[i] = ItemStack.Air;
        for (int i = 0; i < _armor.Length; i++) _armor[i] = ItemStack.Air;
        Offhand = ItemStack.Air;
    }
}
