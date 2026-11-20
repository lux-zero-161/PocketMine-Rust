use pm_math::{BlockPosition, Vector3};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Block {
    pub id: u32,
    pub meta: u16,
}

impl Block {
    pub const AIR: Self = Self { id: 0, meta: 0 };
    pub const STONE: Self = Self { id: 1, meta: 0 };
    pub const GRASS: Self = Self { id: 2, meta: 0 };
    pub const DIRT: Self = Self { id: 3, meta: 0 };
    pub const BEDROCK: Self = Self { id: 7, meta: 0 };

    pub fn new(id: u32, meta: u16) -> Self {
        Self { id, meta }
    }
}

pub const SUB_CHUNK_SIZE: usize = 16 * 16 * 16;

#[derive(Clone)]
pub struct SubChunk {
    pub blocks: Box<[Block; SUB_CHUNK_SIZE]>,
}

impl SubChunk {
    pub fn new() -> Self {
        Self {
            blocks: Box::new([Block::AIR; SUB_CHUNK_SIZE]),
        }
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> Block {
        let index = (x & 15) | ((z & 15) << 4) | ((y & 15) << 8);
        self.blocks[index]
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: Block) {
        let index = (x & 15) | ((z & 15) << 4) | ((y & 15) << 8);
        self.blocks[index] = block;
    }
}

impl Default for SubChunk {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Chunk {
    pub x: i32,
    pub z: i32,
    pub sub_chunks: HashMap<i32, SubChunk>,
}

impl Chunk {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            sub_chunks: HashMap::new(),
        }
    }

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> Block {
        let sub_y = y >> 4;
        if let Some(sub) = self.sub_chunks.get(&sub_y) {
            sub.get_block((x & 15) as usize, (y & 15) as usize, (z & 15) as usize)
        } else {
            Block::AIR
        }
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: Block) {
        let sub_y = y >> 4;
        let sub = self.sub_chunks.entry(sub_y).or_insert_with(SubChunk::new);
        sub.set_block((x & 15) as usize, (y & 15) as usize, (z & 15) as usize, block);
    }
}

pub struct World {
    pub name: String,
    pub spawn: Vector3,
    pub time: RwLock<u64>,
    chunks: RwLock<HashMap<(i32, i32), Chunk>>,
}

impl World {
    pub fn new(name: impl Into<String>) -> Self {
        let world = Self {
            name: name.into(),
            spawn: Vector3::new(0.0, 4.0, 0.0),
            time: RwLock::new(0),
            chunks: RwLock::new(HashMap::new()),
        };
        world.generate_flat_chunk(0, 0);
        world
    }

    pub fn generate_flat_chunk(&self, chunk_x: i32, chunk_z: i32) {
        let mut chunk = Chunk::new(chunk_x, chunk_z);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_block(x, 0, z, Block::BEDROCK);
                chunk.set_block(x, 1, z, Block::DIRT);
                chunk.set_block(x, 2, z, Block::DIRT);
                chunk.set_block(x, 3, z, Block::GRASS);
            }
        }
        let mut map = self.chunks.write().unwrap();
        map.insert((chunk_x, chunk_z), chunk);
    }

    pub fn get_block(&self, pos: &BlockPosition) -> Block {
        let cx = pos.x >> 4;
        let cz = pos.z >> 4;
        let map = self.chunks.read().unwrap();
        if let Some(chunk) = map.get(&(cx, cz)) {
            chunk.get_block(pos.x & 15, pos.y, pos.z & 15)
        } else {
            Block::AIR
        }
    }

    pub fn set_block(&self, pos: &BlockPosition, block: Block) {
        let cx = pos.x >> 4;
        let cz = pos.z >> 4;
        let mut map = self.chunks.write().unwrap();
        let chunk = map.entry((cx, cz)).or_insert_with(|| Chunk::new(cx, cz));
        chunk.set_block(pos.x & 15, pos.y, pos.z & 15, block);
    }

    pub fn tick(&self) {
        let mut time = self.time.write().unwrap();
        *time = (*time + 1) % 24000;
    }
}
