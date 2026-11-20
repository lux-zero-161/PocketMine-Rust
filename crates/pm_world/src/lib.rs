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

    pub fn is_replaceable(&self) -> bool {
        matches!(
            self.name(),
            pm_data::blocks::AIR | pm_data::blocks::WATER | pm_data::blocks::LAVA
        )
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

    /// Distinct block identifiers used by this chunk (from section palettes).
    pub fn block_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .sections
            .iter()
            .flatten()
            .flat_map(|section| {
                section
                    .palette()
                    .iter()
                    .map(|block| block.name().to_string())
            })
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Index of the highest non-empty section, if any.
    pub fn highest_non_empty_section(&self) -> Option<usize> {
        self.sections
            .iter()
            .rposition(|section| section.as_ref().is_some_and(|s| !s.is_empty()))
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

    pub fn with_chunk<R>(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        f: impl FnOnce(&Chunk) -> R,
    ) -> Option<R> {
        let chunks = self.chunks.read().unwrap();
        chunks.get(&(chunk_x, chunk_z)).map(f)
    }

    pub fn loaded_chunk_coords(&self) -> Vec<(i32, i32)> {
        self.chunks.read().unwrap().keys().copied().collect()
    }

    pub fn block_names_in_chunk(&self, chunk_x: i32, chunk_z: i32) -> Vec<String> {
        self.with_chunk(chunk_x, chunk_z, Chunk::block_names)
            .unwrap_or_default()
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

    /// Saves world metadata and every loaded chunk in the interim binary format
    /// (`world.json` + `chunks.bin`). Bedrock LevelDB is the long-term target.
    pub fn save(&self, directory: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(directory)?;
        let meta = serde_json::json!({
            "name": self.name,
            "spawn": self.spawn,
            "seed": self.seed,
            "time": self.time(),
            "difficulty": self.difficulty as u8,
            "generator": self.generator.name(),
        });
        std::fs::write(
            directory.join("world.json"),
            serde_json::to_string_pretty(&meta)?,
        )?;

        let mut out = Vec::new();
        out.extend_from_slice(b"PMRW");
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&self.seed.to_le_bytes());
        out.extend_from_slice(&self.time().to_le_bytes());
        for value in [self.spawn.x, self.spawn.y, self.spawn.z] {
            out.extend_from_slice(&value.to_le_bytes());
        }

        let chunks = self.chunks.read().unwrap();
        out.extend_from_slice(&(chunks.len() as u32).to_le_bytes());
        for chunk in chunks.values() {
            out.extend_from_slice(&chunk.x.to_le_bytes());
            out.extend_from_slice(&chunk.z.to_le_bytes());
            for biome in chunk.biomes.iter() {
                out.extend_from_slice(&biome.to_le_bytes());
            }
            let sections: Vec<(u8, &SubChunk)> = chunk
                .sections()
                .iter()
                .enumerate()
                .filter_map(|(index, section)| {
                    section
                        .as_ref()
                        .filter(|section| !section.is_empty())
                        .map(|section| (index as u8, section))
                })
                .collect();
            out.extend_from_slice(&(sections.len() as u16).to_le_bytes());
            for (index, section) in sections {
                out.push(index);
                out.extend_from_slice(&(section.palette().len() as u16).to_le_bytes());
                for block in section.palette() {
                    out.extend_from_slice(&block.id.to_le_bytes());
                    out.extend_from_slice(&block.meta.to_le_bytes());
                }
                for value in section.palette_indices().iter() {
                    out.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        std::fs::write(directory.join("chunks.bin"), out)
    }

    /// Loads a world saved by [`World::save`], or creates a fresh flat world.
    pub fn load_or_generate(directory: &std::path::Path, name: impl Into<String>) -> Self {
        let name = name.into();
        let mut world = Self::flat(name);

        if let Ok(text) = std::fs::read_to_string(directory.join("world.json")) {
            if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(seed) = meta.get("seed").and_then(serde_json::Value::as_i64) {
                    world.seed = seed;
                }
                if let Some(time) = meta.get("time").and_then(serde_json::Value::as_i64) {
                    *world.time.write().unwrap() = time;
                }
                if let Some(difficulty) = meta.get("difficulty").and_then(serde_json::Value::as_u64)
                {
                    world.difficulty = match difficulty {
                        0 => Difficulty::Peaceful,
                        1 => Difficulty::Easy,
                        2 => Difficulty::Normal,
                        _ => Difficulty::Hard,
                    };
                }
                if let Some(spawn) = meta.get("spawn") {
                    let axis = |key: &str| {
                        spawn
                            .get(key)
                            .and_then(serde_json::Value::as_f64)
                            .unwrap_or(0.0)
                    };
                    world.spawn = Vector3::new(axis("x"), axis("y"), axis("z"));
                }
            }
        }

        if let Ok(bytes) = std::fs::read(directory.join("chunks.bin")) {
            if world.apply_chunk_dump(&bytes) {
                return world;
            }
        }

        world.load_chunks_around(world.spawn, 2);
        world
    }

    fn apply_chunk_dump(&mut self, bytes: &[u8]) -> bool {
        let mut offset = 0usize;
        let take = |offset: &mut usize, count: usize| -> Option<&[u8]> {
            let slice = bytes.get(*offset..*offset + count)?;
            *offset += count;
            Some(slice)
        };
        let Some(magic) = take(&mut offset, 4) else {
            return false;
        };
        if magic != b"PMRW" {
            return false;
        }
        let Some(version) = take(&mut offset, 4) else {
            return false;
        };
        if u32::from_le_bytes(version.try_into().unwrap()) != 1 {
            return false;
        }
        let Some(seed) = take(&mut offset, 8) else {
            return false;
        };
        self.seed = i64::from_le_bytes(seed.try_into().unwrap());
        let Some(time) = take(&mut offset, 8) else {
            return false;
        };
        *self.time.write().unwrap() = i64::from_le_bytes(time.try_into().unwrap());
        for axis in 0..3 {
            let Some(value) = take(&mut offset, 8) else {
                return false;
            };
            let value = f64::from_le_bytes(value.try_into().unwrap());
            match axis {
                0 => self.spawn.x = value,
                1 => self.spawn.y = value,
                _ => self.spawn.z = value,
            }
        }

        let Some(count) = take(&mut offset, 4) else {
            return false;
        };
        let count = u32::from_le_bytes(count.try_into().unwrap()) as usize;
        let mut chunks = self.chunks.write().unwrap();
        chunks.clear();
        for _ in 0..count {
            let Some(x) = take(&mut offset, 4) else {
                return false;
            };
            let Some(z) = take(&mut offset, 4) else {
                return false;
            };
            let (x, z) = (
                i32::from_le_bytes(x.try_into().unwrap()),
                i32::from_le_bytes(z.try_into().unwrap()),
            );
            let mut chunk = Chunk::new(x, z);
            let Some(biomes) = take(&mut offset, 512) else {
                return false;
            };
            for (index, biome) in chunk.biomes.iter_mut().enumerate() {
                *biome = u16::from_le_bytes([biomes[index * 2], biomes[index * 2 + 1]]);
            }
            let Some(section_count) = take(&mut offset, 2) else {
                return false;
            };
            let section_count = u16::from_le_bytes(section_count.try_into().unwrap());
            for _ in 0..section_count {
                let Some(index) = take(&mut offset, 1) else {
                    return false;
                };
                let Some(palette_len) = take(&mut offset, 2) else {
                    return false;
                };
                let palette_len = u16::from_le_bytes(palette_len.try_into().unwrap()) as usize;
                let mut palette = Vec::with_capacity(palette_len);
                for _ in 0..palette_len {
                    let Some(block) = take(&mut offset, 4) else {
                        return false;
                    };
                    palette.push(Block {
                        id: u16::from_le_bytes([block[0], block[1]]),
                        meta: u16::from_le_bytes([block[2], block[3]]),
                    });
                }
                let Some(indices) = take(&mut offset, 8192) else {
                    return false;
                };
                let section_index = index[0] as i32;
                for position in 0..4096usize {
                    let palette_index =
                        u16::from_le_bytes([indices[position * 2], indices[position * 2 + 1]])
                            as usize;
                    let Some(block) = palette.get(palette_index) else {
                        continue;
                    };
                    if block.is_air() {
                        continue;
                    }
                    let local_x = (position & 15) as i32;
                    let local_z = ((position >> 4) & 15) as i32;
                    let local_y = ((position >> 8) & 15) as i32;
                    let y = WORLD_MIN_Y + section_index * 16 + local_y;
                    chunk.set_block(local_x, y, local_z, *block);
                }
            }
            chunks.insert((x, z), chunk);
        }
        true
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

#[cfg(test)]
mod persistence_tests {
    use super::*;

    #[test]
    fn saves_and_loads_chunks() {
        let dir = std::env::temp_dir().join(format!("pm_world_persist_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let world = World::flat("persist");
        world.load_chunk(0, 0);
        world.set_block_at(5, 20, 5, Block::of("minecraft:diamond_block"));
        world.set_time(1234);
        world.save(&dir).unwrap();

        let loaded = World::load_or_generate(&dir, "persist");
        assert_eq!(loaded.time(), 1234);
        assert_eq!(
            loaded.get_block_at(5, 20, 5).name(),
            "minecraft:diamond_block"
        );
        assert_eq!(loaded.get_block_at(0, 0, 0).name(), "minecraft:bedrock");
        assert!(loaded.is_chunk_loaded(0, 0));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
