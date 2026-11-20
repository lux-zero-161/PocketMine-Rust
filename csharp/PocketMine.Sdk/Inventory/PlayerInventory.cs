using PocketMine.Item;

namespace PocketMine.Inventory;

public class PlayerInventory
{
    private readonly ItemStack[] _slots = new ItemStack[36];
    private readonly ItemStack[] _armor = new ItemStack[4];

    public ItemStack Offhand { get; set; } = ItemStack.Air;
    public int HeldSlot { get; set; }
    public int Size => _slots.Length + _armor.Length + 1;

    public PlayerInventory()
    {
        for (int i = 0; i < _slots.Length; i++) _slots[i] = ItemStack.Air;
        for (int i = 0; i < _armor.Length; i++) _armor[i] = ItemStack.Air;
    }

    public ItemStack GetItem(int slot)
    {
        if (slot >= 0 && slot < _slots.Length) return _slots[slot];
        if (slot >= _slots.Length && slot < _slots.Length + _armor.Length) return _armor[slot - _slots.Length];
        return slot == Size - 1 ? Offhand : ItemStack.Air;
    }

    public void SetItem(int slot, ItemStack item)
    {
        if (slot >= 0 && slot < _slots.Length) _slots[slot] = item;
        else if (slot >= _slots.Length && slot < _slots.Length + _armor.Length) _armor[slot - _slots.Length] = item;
        else if (slot == Size - 1) Offhand = item;
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

    public bool RemoveItem(ItemStack item, int amount = 1)
    {
        for (int i = 0; i < _slots.Length && amount > 0; i++)
        {
            if (_slots[i].Equals(item) && !_slots[i].IsEmpty)
            {
                int taken = System.Math.Min(amount, _slots[i].Count);
                _slots[i].Count -= (byte)taken;
                amount -= taken;
                if (_slots[i].Count == 0) _slots[i] = ItemStack.Air;
            }
        }
        return amount == 0;
    }

    public IEnumerable<ItemStack> Contents => _slots;

    public void Clear()
    {
        for (int i = 0; i < _slots.Length; i++) _slots[i] = ItemStack.Air;
        for (int i = 0; i < _armor.Length; i++) _armor[i] = ItemStack.Air;
        Offhand = ItemStack.Air;
    }
}
