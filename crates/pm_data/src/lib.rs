//! Block and item registry, generated from the same `bedrock-data` package that
//! PocketMine-MP uses. Keeping the data identical is what makes item IDs and
//! block properties behave 1:1 with the PHP server.

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct BlockProperties {
    #[serde(default)]
    pub blast_resistance: f64,
    #[serde(default)]
    pub brightness: f64,
    #[serde(default)]
    pub flame_encouragement: i32,
    #[serde(default)]
    pub flammability: i32,
    #[serde(default)]
    pub friction: f64,
    #[serde(default)]
    pub hardness: f64,
    #[serde(default)]
    pub opacity: f64,
    #[serde(skip)]
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ItemType {
    pub numeric_id: i32,
    pub string_id: String,
    pub version: i32,
    pub component_based: bool,
}

static BLOCK_JSON: &str = include_str!("../data/block_properties_table.json");
static ITEM_JSON: &str = include_str!("../data/item_registry.json");
static BLOCK_TO_ITEM_JSON: &str = include_str!("../data/block_id_to_item_id_map.json");

static BLOCKS: OnceLock<HashMap<String, BlockProperties>> = OnceLock::new();
static ITEMS: OnceLock<Vec<ItemType>> = OnceLock::new();
static ITEM_BY_NAME: OnceLock<HashMap<String, ItemType>> = OnceLock::new();
static ITEM_BY_ID: OnceLock<HashMap<i32, ItemType>> = OnceLock::new();
static BLOCK_TO_ITEM: OnceLock<HashMap<String, String>> = OnceLock::new();

fn blocks_map() -> &'static HashMap<String, BlockProperties> {
    BLOCKS.get_or_init(|| {
        let raw: HashMap<String, Value> = serde_json::from_str(BLOCK_JSON).unwrap_or_default();
        let mut out = HashMap::with_capacity(raw.len());
        for (name, value) in raw {
            let mut props: BlockProperties =
                serde_json::from_value(value).unwrap_or(BlockProperties {
                    blast_resistance: 0.0,
                    brightness: 0.0,
                    flame_encouragement: 0,
                    flammability: 0,
                    friction: 0.6,
                    hardness: 0.0,
                    opacity: 1.0,
                    name: String::new(),
                });
            props.name = name.clone();
            out.insert(name, props);
        }
        out
    })
}

fn items_vec() -> &'static Vec<ItemType> {
    ITEMS.get_or_init(|| {
        let raw: Vec<Value> = serde_json::from_str(ITEM_JSON).unwrap_or_default();
        let mut out = Vec::with_capacity(raw.len());
        for entry in raw {
            let get = |suffix: &str| -> Option<&Value> {
                entry
                    .as_object()?
                    .iter()
                    .find(|(k, _)| k.ends_with(suffix))
                    .map(|(_, v)| v)
            };
            let numeric_id = get("numericId").and_then(Value::as_i64).unwrap_or(-1) as i32;
            let string_id = get("stringId")
                .and_then(Value::as_str)
                .unwrap_or("minecraft:unknown")
                .to_string();
            let version = get("version").and_then(Value::as_i64).unwrap_or(0) as i32;
            let component_based = get("componentBased")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            out.push(ItemType {
                numeric_id,
                string_id,
                version,
                component_based,
            });
        }
        out.sort_by_key(|i| i.numeric_id);
        out
    })
}

/// Number of registered block types.
pub fn block_count() -> usize {
    blocks_map().len()
}

/// Properties for a block identifier such as `minecraft:stone`.
pub fn block_properties(name: &str) -> Option<&'static BlockProperties> {
    blocks_map().get(name)
}

pub fn is_block(name: &str) -> bool {
    blocks_map().contains_key(name)
}

/// All block identifiers, useful for tab completion and debugging.
pub fn block_names() -> impl Iterator<Item = &'static str> {
    blocks_map().keys().map(String::as_str)
}

pub fn items() -> &'static [ItemType] {
    items_vec()
}

pub fn item_by_name(name: &str) -> Option<&'static ItemType> {
    let map = ITEM_BY_NAME.get_or_init(|| {
        items_vec()
            .iter()
            .map(|i| (i.string_id.clone(), i.clone()))
            .collect()
    });
    map.get(name)
}

pub fn item_by_id(id: i32) -> Option<&'static ItemType> {
    let map = ITEM_BY_ID.get_or_init(|| {
        items_vec()
            .iter()
            .map(|i| (i.numeric_id, i.clone()))
            .collect()
    });
    map.get(&id)
}

/// Maps a block identifier to the item it drops, using bedrock-data's map.
pub fn item_for_block(block: &str) -> Option<&'static str> {
    let map = BLOCK_TO_ITEM.get_or_init(|| {
        serde_json::from_str::<HashMap<String, String>>(BLOCK_TO_ITEM_JSON).unwrap_or_default()
    });
    map.get(block).map(String::as_str)
}

/// Well known vanilla block identifiers.
pub mod blocks {
    pub const AIR: &str = "minecraft:air";
    pub const STONE: &str = "minecraft:stone";
    pub const GRASS_BLOCK: &str = "minecraft:grass_block";
    pub const DIRT: &str = "minecraft:dirt";
    pub const BEDROCK: &str = "minecraft:bedrock";
    pub const SAND: &str = "minecraft:sand";
    pub const GRAVEL: &str = "minecraft:gravel";
    pub const WATER: &str = "minecraft:water";
    pub const LAVA: &str = "minecraft:lava";
    pub const OAK_LOG: &str = "minecraft:oak_log";
    pub const OAK_PLANKS: &str = "minecraft:oak_planks";
    pub const OAK_LEAVES: &str = "minecraft:oak_leaves";
    pub const GLASS: &str = "minecraft:glass";
    pub const COBBLESTONE: &str = "minecraft:cobblestone";
    pub const TORCH: &str = "minecraft:torch";
    pub const CRAFTING_TABLE: &str = "minecraft:crafting_table";
}

/// Well known vanilla item identifiers.
pub mod items {
    pub const AIR: &str = "minecraft:air";
    pub const DIAMOND: &str = "minecraft:diamond";
    pub const IRON_INGOT: &str = "minecraft:iron_ingot";
    pub const GOLD_INGOT: &str = "minecraft:gold_ingot";
    pub const STICK: &str = "minecraft:stick";
    pub const APPLE: &str = "minecraft:apple";
    pub const BREAD: &str = "minecraft:bread";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_bedrock_data() {
        assert!(block_count() > 500, "expected a full block registry");
        let stone = block_properties("minecraft:stone").expect("stone must exist");
        assert!(stone.hardness > 0.0);
        assert!(items().len() > 1000, "expected a full item registry");
        assert!(item_by_name("minecraft:diamond").is_some());
        assert_eq!(item_for_block("minecraft:stone"), Some("minecraft:stone"));
    }
}
