#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStack {
    pub id: u32,
    pub meta: u16,
    pub count: u8,
    pub custom_name: Option<String>,
}

impl ItemStack {
    pub const AIR: Self = Self {
        id: 0,
        meta: 0,
        count: 0,
        custom_name: None,
    };

    pub fn new(id: u32, meta: u16, count: u8) -> Self {
        Self {
            id,
            meta,
            count,
            custom_name: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.id == 0 || self.count == 0
    }
}

pub const INVENTORY_SIZE: usize = 36;
pub const ARMOR_SIZE: usize = 4;

#[derive(Debug, Clone)]
pub struct PlayerInventory {
    slots: [ItemStack; INVENTORY_SIZE],
    armor: [ItemStack; ARMOR_SIZE],
    offhand: ItemStack,
    held_slot: usize,
}

impl PlayerInventory {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| ItemStack::AIR),
            armor: std::array::from_fn(|_| ItemStack::AIR),
            offhand: ItemStack::AIR,
            held_slot: 0,
        }
    }

    pub fn get_item(&self, slot: usize) -> Option<&ItemStack> {
        self.slots.get(slot)
    }

    pub fn set_item(&mut self, slot: usize, item: ItemStack) -> bool {
        if let Some(target) = self.slots.get_mut(slot) {
            *target = item;
            true
        } else {
            false
        }
    }

    pub fn get_held_item(&self) -> &ItemStack {
        &self.slots[self.held_slot]
    }

    pub fn set_held_slot(&mut self, slot: usize) -> bool {
        if slot < 9 {
            self.held_slot = slot;
            true
        } else {
            false
        }
    }

    pub fn add_item(&mut self, item: ItemStack) -> bool {
        for slot in self.slots.iter_mut() {
            if slot.is_empty() {
                *slot = item;
                return true;
            }
        }
        false
    }

    pub fn clear(&mut self) {
        for slot in self.slots.iter_mut() {
            *slot = ItemStack::AIR;
        }
        for armor in self.armor.iter_mut() {
            *armor = ItemStack::AIR;
        }
        self.offhand = ItemStack::AIR;
    }
}

impl Default for PlayerInventory {
    fn default() -> Self {
        Self::new()
    }
}
