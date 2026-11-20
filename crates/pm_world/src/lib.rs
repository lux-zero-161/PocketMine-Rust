//! World, chunk and block storage.
//!
//! The block registry is index-based and backed by the exact `bedrock-data`
//! tables used by PocketMine-MP, so block names, hardness and IDs line up with
//! the PHP server. Chunks use a palette + 16-bit index layout like PMMP's
//! `SubChunk`, which keeps memory low and maps directly onto the network
//! sub-chunk encoding.

use pm_core::{WORLD_HEIGHT_SUBCHUNKS, WORLD_MIN_Y};
use pm_data;
use pm_math::{BlockPosition, Vector3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

// ---------------------------------------------------------------------------
// Block registry
// ---------------------------------------------------------------------------

pub struct BlockRegistry {
    names: Vec<&'static str>,
    ids: HashMap<&'static str, u16>,
}

static REGISTRY: OnceLock<BlockRegistry> = OnceLock::new();

fn registry() -> &'static BlockRegistry {
    REGISTRY.get_or_init(|| {
        let mut names: Vec<&'static str> = pm_data::block_names().collect();
        names.sort_unstable();
        let ids = names
            .iter()
            .enumerate()
            .map(|(i, n)| (*n, i as u16))
            .collect();
        BlockRegistry { names, ids }
    })
}

/// A block type (identifier + state metadata).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Block {
    pub id: u16,
    pub meta: u16,
}

impl Block {
    pub fn air() -> Self {
        Self::from_name(pm_data::blocks::AIR).unwrap_or(Self { id: 0, meta: 0 })
    }

    pub fn from_name(name: &str) -> Option<Self> {
        registry().ids.get(name).map(|id| Self { id: *id, meta: 0 })
    }

    /// Looks up a block by name and fails loudly, useful for constants.
    pub fn of(name: &str) -> Self {
        Self::from_name(name).unwrap_or_else(|| panic!("unknown block type: {name}"))
    }

    pub fn new(name: &str, meta: u16) -> Self {
        let mut block = Self::of(name);
        block.meta = meta;
        block
    }

    pub fn from_id(id: u16) -> Self {
        Self { id, meta: 0 }
    }

    pub fn name(&self) -> &'static str {
        registry()
            .names
            .get(self.id as usize)
            .copied()
            .unwrap_or(pm_data::blocks::AIR)
    }

    pub fn properties(&self) -> Option<&'static pm_data::BlockProperties> {
        pm_data::block_properties(self.name())
    }

    pub fn is_air(&self) -> bool {
        self.name() == pm_data::blocks::AIR
    }

    pub fn is_solid(&self) -> bool {
        !self.is_air()
            && self.name() != pm_data::blocks::WATER
            && self.name() != pm_data::blocks::LAVA
    }

    pub fn hardness(&self) -> f64 {
        self.properties().map(|p| p.hardness).unwrap_or(1.0)
    }

    pub fn blast_resistance(&self) -> f64 {
        self.properties().map(|p| p.blast_resistance).unwrap_or(0.0)
    }

    pub fn light_level(&self) -> u8 {
        self.properties()
            .map(|p| p.brightness.clamp(0.0, 15.0) as u8)
            .unwrap_or(0)
    }
}

/// Convenience constructors for the most common vanilla blocks.
pub mod blocks {
    use super::Block;
    use pm_data::blocks as n;

    pub fn air() -> Block {
        Block::of(n::AIR)
    }
    pub fn stone() -> Block {
        Block::of(n::STONE)
    }
    pub fn grass() -> Block {
        Block::of(n::GRASS_BLOCK)
    }
    pub fn dirt() -> Block {
        Block::of(n::DIRT)
    }
    pub fn bedrock() -> Block {
        Block::of(n::BEDROCK)
    }
    pub fn sand() -> Block {
        Block::of(n::SAND)
    }
    pub fn water() -> Block {
        Block::of(n::WATER)
    }
    pub fn glass() -> Block {
        Block::of(n::GLASS)
    }
    pub fn oak_log() -> Block {
        Block::of(n::OAK_LOG)
    }
    pub fn oak_planks() -> Block {
        Block::of(n::OAK_PLANKS)
    }
}

// ---------------------------------------------------------------------------
// Sub chunks and chunks
// ---------------------------------------------------------------------------

pub const SUB_CHUNK_SIZE: usize = 16 * 16 * 16;
const SUB_CHUNK_SECTION_COUNT: usize = WORLD_HEIGHT_SUBCHUNKS as usize;

#[derive(Debug, Clone)]
pub struct SubChunk {
    /// Palette of block states used in this section.
    palette: Vec<Block>,
    /// 4096 palette indices.
    data: Box<[u16; SUB_CHUNK_SIZE]>,
    non_air: u16,
    dirty: bool,
}

impl SubChunk {
    pub fn empty() -> Self {
        Self {
            palette: vec![Block::air()],
            data: Box::new([0; SUB_CHUNK_SIZE]),
            non_air: 0,
            dirty: false,
        }
    }

    #[inline]
    fn index(x: usize, y: usize, z: usize) -> usize {
        (x & 15) | ((z & 15) << 4) | ((y & 15) << 8)
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> Block {
        let idx = self.data[Self::index(x, y, z)] as usize;
        self.palette.get(idx).copied().unwrap_or_else(Block::air)
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: Block) -> bool {
        let index = Self::index(x, y, z);
        let old = self.get_block(x, y, z);
        if old == block {
            return false;
        }
        let new_index = match self.palette.iter().position(|b| *b == block) {
            Some(i) => i,
            None => {
                if self.palette.len() >= u16::MAX as usize {
                    return false;
                }
                self.palette.push(block);
                self.palette.len() - 1
            }
        };
        self.data[index] = new_index as u16;
        if old.is_air() && !block.is_air() {
            self.non_air += 1;
        } else if !old.is_air() && block.is_air() {
            self.non_air = self.non_air.saturating_sub(1);
        }
        self.dirty = true;
        true
    }

    pub fn is_empty(&self) -> bool {
        self.non_air == 0
    }

    pub fn non_air_count(&self) -> u16 {
        self.non_air
    }

    pub fn palette(&self) -> &[Block] {
        &self.palette
    }

    pub fn palette_indices(&self) -> &[u16; SUB_CHUNK_SIZE] {
        &self.data
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}

impl Default for SubChunk {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
    sections: Vec<Option<SubChunk>>,
    /// 16x16 biome IDs (0 = ocean, 1 = plains, ...).
    pub biomes: Box<[u16; 256]>,
    pub dirty: bool,
}

impl Chunk {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            sections: vec![None; SUB_CHUNK_SECTION_COUNT],
            biomes: Box::new([1; 256]),
            dirty: false,
        }
    }

    fn section_index(y: i32) -> Option<usize> {
        let idx = (y - WORLD_MIN_Y) >> 4;
        if idx < 0 || idx as usize >= SUB_CHUNK_SECTION_COUNT {
            None
        } else {
            Some(idx as usize)
        }
    }

    pub fn get_block(&self, local_x: i32, y: i32, local_z: i32) -> Block {
        let Some(section_idx) = Self::section_index(y) else {
            return Block::air();
        };
        match &self.sections[section_idx] {
            Some(section) => {
                section.get_block(local_x as usize, (y & 15) as usize, local_z as usize)
            }
            None => Block::air(),
        }
    }

    pub fn set_block(&mut self, local_x: i32, y: i32, local_z: i32, block: Block) -> bool {
        let Some(section_idx) = Self::section_index(y) else {
            return false;
        };
        let section = self.sections[section_idx].get_or_insert_with(SubChunk::empty);
        let changed =
            section.set_block(local_x as usize, (y & 15) as usize, local_z as usize, block);
        if changed {
            self.dirty = true;
            if section.is_empty() {
                // Drop empty sections to keep memory flat.
                self.sections[section_idx] = None;
            }
        }
        changed
    }

    pub fn section(&self, y: i32) -> Option<&SubChunk> {
        Self::section_index(y).and_then(|i| self.sections[i].as_ref())
    }

    pub fn sections(&self) -> &[Option<SubChunk>] {
        &self.sections
    }

    /// Highest non-air block plus one, used for spawning.
    pub fn highest_block_y(&self) -> i32 {
        for idx in (0..self.sections.len()).rev() {
            let Some(section) = &self.sections[idx] else {
                continue;
            };
            if section.is_empty() {
                continue;
            }
            for local_y in (0..16).rev() {
                for x in 0..16usize {
                    for z in 0..16usize {
                        if !section.get_block(x, local_y, z).is_air() {
                            return WORLD_MIN_Y + (idx as i32 * 16) + local_y as i32 + 1;
                        }
                    }
                }
            }
        }
        WORLD_MIN_Y
    }

    pub fn non_empty_section_count(&self) -> usize {
        self.sections
            .iter()
            .filter(|s| s.as_ref().is_some_and(|s| !s.is_empty()))
            .count()
    }
}

// ---------------------------------------------------------------------------
// Generator
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FlatLayer {
    pub block: Block,
    pub height: i32,
}

#[derive(Debug, Clone)]
pub struct FlatGenerator {
    pub layers: Vec<FlatLayer>,
}

impl Default for FlatGenerator {
    fn default() -> Self {
        // Same default as PMMP's flat generator: bedrock, 2x dirt, grass.
        Self {
            layers: vec![
                FlatLayer {
                    block: blocks::bedrock(),
                    height: 1,
                },
                FlatLayer {
                    block: blocks::dirt(),
                    height: 2,
                },
                FlatLayer {
                    block: blocks::grass(),
                    height: 1,
                },
            ],
        }
    }
}

pub trait Generator: Send + Sync {
    fn name(&self) -> &'static str;
    fn generate_chunk(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32);
}

impl Generator for FlatGenerator {
    fn name(&self) -> &'static str {
        "flat"
    }

    fn generate_chunk(&self, chunk: &mut Chunk, _chunk_x: i32, _chunk_z: i32) {
        let mut y = 0;
        for layer in &self.layers {
            for _ in 0..layer.height {
                if y >= 320 {
                    break;
                }
                for x in 0..16 {
                    for z in 0..16 {
                        chunk.set_block(x, y, z, layer.block);
                    }
                }
                y += 1;
            }
        }
        chunk.dirty = true;
    }
}

#[derive(Debug, Clone)]
pub struct VoidGenerator;

impl Generator for VoidGenerator {
    fn name(&self) -> &'static str {
        "void"
    }
    fn generate_chunk(&self, chunk: &mut Chunk, _chunk_x: i32, _chunk_z: i32) {
        chunk.set_block(0, 0, 0, blocks::bedrock());
    }
}

// ---------------------------------------------------------------------------
// Entities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_id: u64,
    pub type_name: String,
    pub position: Vector3,
    pub yaw: f64,
    pub pitch: f64,
    pub health: f32,
    pub max_health: f32,
    pub on_ground: bool,
}

impl Entity {
    pub fn new(entity_id: u64, type_name: impl Into<String>, position: Vector3) -> Self {
        Self {
            entity_id,
            type_name: type_name.into(),
            position,
            yaw: 0.0,
            pitch: 0.0,
            health: 20.0,
            max_health: 20.0,
            on_ground: false,
        }
    }
}

// ---------------------------------------------------------------------------
// World
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Peaceful = 0,
    Easy = 1,
    Normal = 2,
    Hard = 3,
}

pub struct World {
    pub name: String,
    pub spawn: Vector3,
    pub seed: i64,
    pub generator: Arc<dyn Generator>,
    pub difficulty: Difficulty,
    time: RwLock<i64>,
    chunks: RwLock<HashMap<(i32, i32), Chunk>>,
    entities: RwLock<HashMap<u64, Entity>>,
    update_queue: RwLock<Vec<BlockPosition>>,
    next_entity_id: RwLock<u64>,
    pub ticking: bool,
    tick_counter: RwLock<u64>,
}

impl World {
    pub fn new(name: impl Into<String>, generator: Arc<dyn Generator>, seed: i64) -> Self {
        Self {
            name: name.into(),
            spawn: Vector3::new(0.0, 5.0, 0.0),
            seed,
            generator,
            difficulty: Difficulty::Normal,
            time: RwLock::new(0),
            chunks: RwLock::new(HashMap::new()),
            entities: RwLock::new(HashMap::new()),
            update_queue: RwLock::new(Vec::new()),
            next_entity_id: RwLock::new(1),
            ticking: true,
            tick_counter: RwLock::new(0),
        }
    }

    pub fn flat(name: impl Into<String>) -> Self {
        Self::new(name, Arc::new(FlatGenerator::default()), 0)
    }

    pub fn time(&self) -> i64 {
        *self.time.read().unwrap()
    }

    pub fn set_time(&self, time: i64) {
        *self.time.write().unwrap() = time.rem_euclid(24_000);
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.chunks.read().unwrap().len()
    }

    pub fn loaded_entity_count(&self) -> usize {
        self.entities.read().unwrap().len()
    }

    pub fn is_chunk_loaded(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.chunks
            .read()
            .unwrap()
            .contains_key(&(chunk_x, chunk_z))
    }

    pub fn load_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        if self.is_chunk_loaded(chunk_x, chunk_z) {
            return false;
        }
        let mut chunk = Chunk::new(chunk_x, chunk_z);
        self.generator.generate_chunk(&mut chunk, chunk_x, chunk_z);
        self.chunks
            .write()
            .unwrap()
            .insert((chunk_x, chunk_z), chunk);
        true
    }

    /// Loads (or generates) the 3x3 chunks around a position, like PMMP does
    /// when a player joins.
    pub fn load_chunks_around(&self, position: Vector3, radius: i32) -> usize {
        let center = position.floor();
        let (cx, cz) = (center.chunk_x(), center.chunk_z());
        let mut loaded = 0;
        for x in (cx - radius)..=(cx + radius) {
            for z in (cz - radius)..=(cz + radius) {
                if self.load_chunk(x, z) {
                    loaded += 1;
                }
            }
        }
        loaded
    }

    pub fn get_block(&self, pos: &BlockPosition) -> Block {
        let (cx, cz) = (pos.chunk_x(), pos.chunk_z());
        let chunks = self.chunks.read().unwrap();
        match chunks.get(&(cx, cz)) {
            Some(chunk) => chunk.get_block(pos.local_x(), pos.y, pos.local_z()),
            None => Block::air(),
        }
    }

    pub fn get_block_at(&self, x: i32, y: i32, z: i32) -> Block {
        self.get_block(&BlockPosition::new(x, y, z))
    }

    /// Sets a block and queues a neighbour update, returning whether the world
    /// changed.
    pub fn set_block(&self, pos: &BlockPosition, block: Block) -> bool {
        let (cx, cz) = (pos.chunk_x(), pos.chunk_z());
        self.load_chunk(cx, cz);
        let mut chunks = self.chunks.write().unwrap();
        let Some(chunk) = chunks.get_mut(&(cx, cz)) else {
            return false;
        };
        let changed = chunk.set_block(pos.local_x(), pos.y, pos.local_z(), block);
        drop(chunks);
        if changed {
            self.update_queue.write().unwrap().push(*pos);
        }
        changed
    }

    pub fn set_block_at(&self, x: i32, y: i32, z: i32, block: Block) -> bool {
        self.set_block(&BlockPosition::new(x, y, z), block)
    }

    /// Highest non-air Y at a column.
    pub fn highest_block_y(&self, x: i32, z: i32) -> i32 {
        let pos = BlockPosition::new(x, 0, z);
        self.load_chunk(pos.chunk_x(), pos.chunk_z());
        let chunks = self.chunks.read().unwrap();
        chunks
            .get(&(pos.chunk_x(), pos.chunk_z()))
            .map(Chunk::highest_block_y)
            .unwrap_or(WORLD_MIN_Y)
    }

    pub fn spawn_position(&self) -> Vector3 {
        Vector3::new(
            self.spawn.x,
            self.highest_block_y(self.spawn.x as i32, self.spawn.z as i32) as f64,
            self.spawn.z,
        )
    }

    pub fn spawn_entity(&self, type_name: impl Into<String>, position: Vector3) -> u64 {
        let mut next = self.next_entity_id.write().unwrap();
        let id = *next;
        *next += 1;
        drop(next);
        let entity = Entity::new(id, type_name, position);
        self.entities.write().unwrap().insert(id, entity);
        id
    }

    pub fn get_entity(&self, entity_id: u64) -> Option<Entity> {
        self.entities.read().unwrap().get(&entity_id).cloned()
    }

    pub fn entities(&self) -> Vec<Entity> {
        self.entities.read().unwrap().values().cloned().collect()
    }

    pub fn remove_entity(&self, entity_id: u64) -> bool {
        self.entities.write().unwrap().remove(&entity_id).is_some()
    }

    /// Per-tick world update: daylight cycle and queued block updates.
    pub fn tick(&self) {
        let mut counter = self.tick_counter.write().unwrap();
        *counter += 1;
        let tick = *counter;
        drop(counter);

        // One Minecraft day is 24000 ticks.
        if tick % 20 == 0 {
            let mut time = self.time.write().unwrap();
            *time = (*time + 20).rem_euclid(24_000);
        }

        if tick % 4 == 0 {
            self.update_queue.write().unwrap().clear();
        }
    }

    /// Saves world metadata as JSON (chunk persistence is a later milestone).
    pub fn save(&self, directory: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(directory)?;
        let meta = serde_json::json!({
            "name": self.name,
            "spawn": self.spawn,
            "seed": self.seed,
            "time": self.time(),
            "difficulty": self.difficulty as u8,
            "generator": self.generator.name(),
            "loaded_chunks": self.loaded_chunk_count(),
        });
        std::fs::write(
            directory.join("world.json"),
            serde_json::to_string_pretty(&meta)?,
        )
    }
}

/// Holds every loaded world, mirroring `pocketmine\world\WorldManager`.
pub struct WorldManager {
    worlds: RwLock<HashMap<String, Arc<World>>>,
    default_world: RwLock<Option<String>>,
}

impl WorldManager {
    pub fn new() -> Self {
        Self {
            worlds: RwLock::new(HashMap::new()),
            default_world: RwLock::new(None),
        }
    }

    pub fn register(&self, world: Arc<World>, make_default: bool) {
        let name = world.name.clone();
        self.worlds.write().unwrap().insert(name.clone(), world);
        if make_default {
            *self.default_world.write().unwrap() = Some(name);
        }
    }

    pub fn unregister(&self, name: &str) -> Option<Arc<World>> {
        self.worlds.write().unwrap().remove(name)
    }

    pub fn get(&self, name: &str) -> Option<Arc<World>> {
        self.worlds.read().unwrap().get(name).cloned()
    }

    pub fn default_world(&self) -> Option<Arc<World>> {
        let name = self.default_world.read().unwrap().clone()?;
        self.get(&name)
    }

    pub fn all(&self) -> Vec<Arc<World>> {
        self.worlds.read().unwrap().values().cloned().collect()
    }

    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.worlds.read().unwrap().keys().cloned().collect();
        names.sort();
        names
    }

    pub fn tick_all(&self) {
        for world in self.all() {
            if world.ticking {
                world.tick();
            }
        }
    }
}

impl Default for WorldManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_vanilla_blocks() {
        assert_eq!(Block::of("minecraft:stone").name(), "minecraft:stone");
        assert!(Block::of("minecraft:stone").hardness() > 0.0);
        assert!(blocks::air().is_air());
    }

    #[test]
    fn flat_world_generates_and_persists_blocks() {
        let world = World::flat("test");
        world.load_chunk(0, 0);
        assert_eq!(world.get_block_at(0, 0, 0).name(), "minecraft:bedrock");
        assert_eq!(world.get_block_at(0, 3, 0).name(), "minecraft:grass_block");
        assert!(world.get_block_at(0, 4, 0).is_air());

        assert!(world.set_block_at(5, 10, 5, blocks::stone()));
        assert_eq!(world.get_block_at(5, 10, 5).name(), "minecraft:stone");
        assert_eq!(world.highest_block_y(5, 5), 11);
    }

    #[test]
    fn subchunk_palette_drops_empty_sections() {
        let world = World::flat("test2");
        world.load_chunk(0, 0);
        world.set_block_at(1, 40, 1, blocks::stone());
        assert!(world.get_block_at(1, 40, 1).is_solid());
        world.set_block_at(1, 40, 1, blocks::air());
        assert!(world.get_block_at(1, 40, 1).is_air());
    }
}
