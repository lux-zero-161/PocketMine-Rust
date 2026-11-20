//! Bedrock game-packet encoders for the post-login phase.
//!
//! These are byte-for-byte implementations of the 1.26.60 wire format
//! (`StartGamePacket`, `LevelSettings`, `LevelChunkPacket`,
//! `ChunkSerializer` from PocketMine-MP).

use pm_math::BlockPosition;
use pm_nbt;
use pm_network::{write_varint, write_varint_signed};
use pm_player::Player;
use pm_world::{Block, Chunk, SubChunk, World};
use std::collections::{BTreeSet, HashMap};

pub const START_GAME_PACKET: u32 = 0x0b;
pub const LEVEL_CHUNK_PACKET: u32 = 0x3a;
pub const AVAILABLE_ACTOR_IDENTIFIERS_PACKET: u32 = 0x77;
pub const BIOME_DEFINITION_LIST_PACKET: u32 = 0x7a;
pub const ITEM_REGISTRY_PACKET: u32 = 0xa2;
pub const JIGSAW_STRUCTURE_DATA_PACKET: u32 = 0x139;
pub const VOXEL_SHAPES_PACKET: u32 = 0x151;

pub const BLOCK_STATE_VERSION: i32 = 18168865;

fn put_bool(out: &mut Vec<u8>, value: bool) {
    out.push(value as u8);
}

fn put_string(out: &mut Vec<u8>, value: &str) {
    write_varint(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

fn put_byte_array(out: &mut Vec<u8>, value: &[u8]) {
    write_varint(out, value.len() as u32);
    out.extend_from_slice(value);
}

fn put_varlong_signed(out: &mut Vec<u8>, value: i64) {
    write_varint64(out, ((value << 1) ^ (value >> 63)) as u64);
}

fn put_varlong_unsigned(out: &mut Vec<u8>, value: u64) {
    write_varint64(out, value);
}

fn write_varint64(out: &mut Vec<u8>, mut value: u64) {
    loop {
        if value & !0x7f == 0 {
            out.push(value as u8);
            return;
        }
        out.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

fn put_block_position(out: &mut Vec<u8>, position: BlockPosition) {
    write_varint_signed(out, position.x);
    write_varint_signed(out, position.y);
    write_varint_signed(out, position.z);
}

fn put_nil_uuid(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0u8; 16]);
}

fn packet(id: u32, body: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 3);
    write_varint(&mut out, id);
    out.extend_from_slice(&body);
    out
}

// ---------------------------------------------------------------------------
// Block palette
// ---------------------------------------------------------------------------

/// Runtime block palette sent in `StartGame`; chunk storages reference these
/// indices. Scanning the loaded world keeps arbitrary plugin-placed blocks
/// renderable.
pub struct BlockPalette {
    pub entries: Vec<(String, Vec<u8>)>,
    ids: HashMap<String, i32>,
}

impl BlockPalette {
    pub fn build(world: &World) -> Self {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for name in [
            pm_data::blocks::AIR,
            pm_data::blocks::STONE,
            pm_data::blocks::GRASS_BLOCK,
            pm_data::blocks::DIRT,
            pm_data::blocks::BEDROCK,
            pm_data::blocks::SAND,
            pm_data::blocks::GRAVEL,
            pm_data::blocks::WATER,
            pm_data::blocks::LAVA,
            pm_data::blocks::OAK_LOG,
            pm_data::blocks::OAK_PLANKS,
            pm_data::blocks::OAK_LEAVES,
            pm_data::blocks::GLASS,
            pm_data::blocks::COBBLESTONE,
            pm_data::blocks::TORCH,
            pm_data::blocks::CRAFTING_TABLE,
        ] {
            names.insert(name.to_string());
        }
        for (chunk_x, chunk_z) in world.loaded_chunk_coords() {
            for name in world.block_names_in_chunk(chunk_x, chunk_z) {
                names.insert(name);
            }
        }
        names.remove(pm_data::blocks::AIR);

        let mut entries = Vec::with_capacity(names.len() + 1);
        let mut ids = HashMap::with_capacity(names.len() + 1);
        entries.push((
            pm_data::blocks::AIR.to_string(),
            pm_nbt::block_state(pm_data::blocks::AIR, BLOCK_STATE_VERSION),
        ));
        ids.insert(pm_data::blocks::AIR.to_string(), 0);
        for name in names {
            let id = entries.len() as i32;
            ids.insert(name.clone(), id);
            entries.push((
                name.clone(),
                pm_nbt::block_state(&name, BLOCK_STATE_VERSION),
            ));
        }
        Self { entries, ids }
    }

    pub fn id(&self, block: &Block) -> i32 {
        *self
            .ids
            .get(block.name())
            .or_else(|| self.ids.get(pm_data::blocks::STONE))
            .unwrap_or(&0)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

// ---------------------------------------------------------------------------
// StartGame
// ---------------------------------------------------------------------------

pub fn start_game_packet(
    player: &Player,
    world: &World,
    tick: u64,
    motd: &str,
    palette: &BlockPalette,
) -> Vec<u8> {
    let mut body = Vec::with_capacity(2048);
    let position = player.position();
    let gamemode = player.gamemode() as i32;
    let spawn = world.spawn_position().floor();

    put_varlong_signed(&mut body, player.entity_id as i64);
    put_varlong_unsigned(&mut body, player.entity_id);
    write_varint_signed(&mut body, gamemode);
    body.extend_from_slice(&(position.x as f32).to_le_bytes());
    body.extend_from_slice(&(position.y as f32).to_le_bytes());
    body.extend_from_slice(&(position.z as f32).to_le_bytes());
    body.extend_from_slice(&(*player.pitch.read().unwrap() as f32).to_le_bytes());
    body.extend_from_slice(&(*player.yaw.read().unwrap() as f32).to_le_bytes());

    // LevelSettings
    body.extend_from_slice(&(world.seed as u64).to_le_bytes());
    body.extend_from_slice(&0u16.to_le_bytes()); // biome type
    put_string(&mut body, ""); // biome name
    write_varint_signed(&mut body, 0); // dimension: overworld
    write_varint_signed(&mut body, 2); // generator: flat
    write_varint_signed(&mut body, gamemode);
    put_bool(&mut body, false); // hardcore
    write_varint_signed(&mut body, world.difficulty as i32);
    put_block_position(&mut body, spawn);
    put_bool(&mut body, true); // achievements disabled
    write_varint_signed(&mut body, 0); // editor world type
    put_bool(&mut body, false); // created in editor
    put_bool(&mut body, false); // exported from editor
    body.push(0); // editor level migration version (signed byte)
    write_varint_signed(&mut body, world.time() as i32);
    write_varint(&mut body, 0); // education edition offer (unsigned varint)
    put_bool(&mut body, true); // education features enabled
    put_string(&mut body, ""); // education product uuid
    body.extend_from_slice(&0f32.to_le_bytes()); // rain level
    body.extend_from_slice(&0f32.to_le_bytes()); // lightning level
    put_bool(&mut body, false); // confirmed platform locked content
    put_bool(&mut body, true); // multiplayer game
    put_bool(&mut body, false); // LAN broadcast
    write_varint_signed(&mut body, 0); // xbox live broadcast mode
    write_varint_signed(&mut body, 0); // platform broadcast mode
    put_bool(&mut body, true); // commands enabled
    put_bool(&mut body, false); // texture packs required
    write_varint(&mut body, 0); // game rules
    body.extend_from_slice(&0u32.to_le_bytes()); // experiments count (LE)
    put_bool(&mut body, false); // has previously used experiments
    put_bool(&mut body, false); // bonus chest
    put_bool(&mut body, false); // start with map
    body.push(1); // default player permission: member (signed byte)
    body.extend_from_slice(&4i32.to_le_bytes()); // server chunk tick radius
    put_bool(&mut body, false); // locked behaviour pack
    put_bool(&mut body, false); // locked resource pack
    put_bool(&mut body, false); // from locked world template
    put_bool(&mut body, false); // MSA gamertags only
    put_bool(&mut body, false); // from world template
    put_bool(&mut body, false); // world template option locked
    put_bool(&mut body, false); // only spawn v1 villagers
    put_bool(&mut body, false); // disable persona
    put_bool(&mut body, false); // disable custom skins
    put_bool(&mut body, false); // mute emote announcements
    put_string(&mut body, pm_core::MINECRAFT_VERSION_NETWORK);
    body.extend_from_slice(&0i32.to_le_bytes()); // limited world width
    body.extend_from_slice(&0i32.to_le_bytes()); // limited world length
    put_bool(&mut body, false); // is new nether
    put_string(&mut body, ""); // education URI button
    put_string(&mut body, ""); // education URI link
    put_bool(&mut body, false); // experimental gameplay override present
    body.push(0); // chat restriction level
    put_bool(&mut body, false); // disable player interactions
    write_varint_signed(&mut body, 0); // server editor connection policy
    put_bool(&mut body, false); // allow anonymous block drops in editor

    put_string(&mut body, &world.name); // level id
    put_string(&mut body, motd); // world name
    put_string(&mut body, ""); // premium world template id
    put_bool(&mut body, false); // is trial
    write_varint_signed(&mut body, 0); // rewind history size
    put_bool(&mut body, true); // server authoritative block breaking
    body.extend_from_slice(&tick.to_le_bytes());
    write_varint_signed(&mut body, 0); // enchantment seed

    write_varint(&mut body, palette.entries.len() as u32);
    for (name, states) in &palette.entries {
        put_string(&mut body, name);
        put_byte_array(&mut body, states);
    }

    put_string(&mut body, ""); // multiplayer correlation id
    put_bool(&mut body, true); // enable new inventory system
    put_string(
        &mut body,
        &format!("{} {}", pm_core::NAME, pm_core::VERSION),
    );
    put_byte_array(&mut body, &pm_nbt::empty_compound()); // player actor properties
    body.extend_from_slice(&0u64.to_le_bytes()); // block palette checksum
    put_nil_uuid(&mut body);
    put_bool(&mut body, false); // enable client side chunk generation
    put_bool(&mut body, false); // block network ids are hashes
    put_bool(&mut body, true); // network permissions: disable client sounds
    put_bool(&mut body, false); // server join information present
    put_string(&mut body, ""); // telemetry server id
    put_string(&mut body, ""); // telemetry scenario id
    put_string(&mut body, ""); // telemetry world id
    put_string(&mut body, ""); // telemetry owner id

    packet(START_GAME_PACKET, body)
}

// ---------------------------------------------------------------------------
// LevelChunk
// ---------------------------------------------------------------------------

pub fn level_chunk_packet(
    world: &World,
    chunk_x: i32,
    chunk_z: i32,
    palette: &BlockPalette,
) -> Option<Vec<u8>> {
    let (payload, count) = world.with_chunk(chunk_x, chunk_z, |chunk| {
        (serialize_full_chunk(chunk, palette), sub_chunk_count(chunk))
    })?;

    let mut body = Vec::with_capacity(payload.len() + 32);
    write_varint_signed(&mut body, chunk_x);
    write_varint_signed(&mut body, chunk_z);
    write_varint_signed(&mut body, 0); // dimension: overworld
    write_varint(&mut body, count);
    put_bool(&mut body, false); // no sub-chunk request limit
    put_bool(&mut body, false); // cache disabled
    write_varint(&mut body, 0); // blob hashes
    put_byte_array(&mut body, &payload);
    put_bool(&mut body, false); // client biome update
    Some(packet(LEVEL_CHUNK_PACKET, body))
}

pub fn serialize_full_chunk(chunk: &Chunk, palette: &BlockPalette) -> Vec<u8> {
    let count = sub_chunk_count(chunk);
    let mut out = Vec::with_capacity(16 * 1024);
    for index in 0..count as usize {
        serialize_sub_chunk(
            chunk.sections().get(index).and_then(Option::as_ref),
            palette,
            &mut out,
        );
    }
    // Biomes: all 24 overworld sections, single-value palettes.
    for _ in 0..24 {
        out.push(1); // bits per block 0, non-persistent
        write_varint_signed(&mut out, 1); // plains
    }
    out.push(0); // border blocks
    write_varint(&mut out, 0); // tiles
    out
}

fn sub_chunk_count(chunk: &Chunk) -> u32 {
    match chunk.highest_non_empty_section() {
        Some(index) => index as u32 + 1 + 4, // sections start at y = -64 (index -4)
        None => 0,
    }
}

fn serialize_sub_chunk(section: Option<&SubChunk>, palette: &BlockPalette, out: &mut Vec<u8>) {
    out.push(8); // sub-chunk version
    out.push(1); // layer count
    let Some(section) = section.filter(|section| !section.is_empty()) else {
        out.push(1); // bits 0, non-persistent
        write_varint_signed(out, palette.id(&Block::air()));
        return;
    };

    let mut network_ids: Vec<i32> = Vec::new();
    let mut local_to_network: Vec<i32> = Vec::with_capacity(section.palette().len());
    for block in section.palette() {
        let id = palette.id(block);
        let position = match network_ids.iter().position(|existing| *existing == id) {
            Some(position) => position,
            None => {
                network_ids.push(id);
                network_ids.len() - 1
            }
        };
        local_to_network.push(position as i32);
    }

    let bits = if network_ids.len() <= 1 {
        0
    } else {
        (usize::BITS - (network_ids.len() - 1).leading_zeros()).clamp(1, 16) as u8
    };

    out.push((bits << 1) | 1);
    if bits == 0 {
        write_varint_signed(out, network_ids[0]);
        return;
    }

    let blocks_per_word = 32 / bits as usize;
    let word_count = (4096 + blocks_per_word - 1) / blocks_per_word;
    let mut words = vec![0u32; word_count];
    for index in 0..4096usize {
        let local = section.palette_indices()[index] as usize;
        let network = local_to_network.get(local).copied().unwrap_or(0) as u32;
        let word = index / blocks_per_word;
        let shift = (index % blocks_per_word) * bits as usize;
        words[word] |= network << shift;
    }
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }

    write_varint_signed(out, network_ids.len() as i32);
    for id in network_ids {
        write_varint_signed(out, id);
    }
}

// ---------------------------------------------------------------------------
// Supporting packets
// ---------------------------------------------------------------------------

pub fn jigsaw_structure_data_packet() -> Vec<u8> {
    let mut body = Vec::new();
    put_byte_array(&mut body, pm_data::jigsaw_structures_nbt());
    packet(JIGSAW_STRUCTURE_DATA_PACKET, body)
}

pub fn voxel_shapes_packet() -> Vec<u8> {
    let mut body = Vec::new();
    write_varint(&mut body, 0); // shapes
    write_varint(&mut body, 0); // name map
    body.extend_from_slice(&0u16.to_le_bytes()); // custom shape count
    packet(VOXEL_SHAPES_PACKET, body)
}

pub fn item_registry_packet() -> Vec<u8> {
    // Same registry data as the PHP server; component NBT is empty until the
    // item-component port lands, which is enough for the vanilla item list.
    let items = pm_data::items();
    let mut body = Vec::with_capacity(items.len() * 48);
    write_varint(&mut body, items.len() as u32);
    for item in items {
        put_string(&mut body, &item.string_id);
        body.extend_from_slice(&(item.numeric_id as i16).to_le_bytes());
        put_bool(&mut body, item.component_based);
        write_varint_signed(&mut body, item.version);
        put_byte_array(&mut body, &pm_nbt::empty_compound());
    }
    packet(ITEM_REGISTRY_PACKET, body)
}

pub fn available_actor_identifiers_packet() -> Vec<u8> {
    let mut body = Vec::new();
    put_byte_array(&mut body, pm_data::entity_identifiers_nbt());
    packet(AVAILABLE_ACTOR_IDENTIFIERS_PACKET, body)
}

pub fn biome_definition_list_packet() -> Vec<u8> {
    // A single plains definition matching the biome id used by chunks.
    const TAGS: [&str; 5] = ["animal", "monster", "overworld", "plains", "bee_habitat"];
    let mut body = Vec::new();
    write_varint(&mut body, 1); // definitions
    body.extend_from_slice(&0u16.to_le_bytes()); // name index
    body.extend_from_slice(&1u16.to_le_bytes()); // biome id: plains
    body.extend_from_slice(&0.8f32.to_le_bytes()); // temperature
    body.extend_from_slice(&0.4f32.to_le_bytes()); // downfall
    body.extend_from_slice(&0.0f32.to_le_bytes()); // foliage snow
    body.extend_from_slice(&0.125f32.to_le_bytes()); // depth
    body.extend_from_slice(&0.05f32.to_le_bytes()); // scale
    body.extend_from_slice(&0xA544AFF5u32.to_le_bytes()); // ARGB water colour
    put_bool(&mut body, true); // rain
    put_bool(&mut body, true); // tags present
    write_varint(&mut body, TAGS.len() as u32);
    for index in 0..TAGS.len() {
        body.extend_from_slice(&((index + 1) as u16).to_le_bytes());
    }
    put_bool(&mut body, false); // no chunk generation data

    write_varint(&mut body, (TAGS.len() + 1) as u32); // strings
    put_string(&mut body, "minecraft:plains");
    for tag in TAGS {
        put_string(&mut body, tag);
    }
    packet(BIOME_DEFINITION_LIST_PACKET, body)
}

/// Packets sent right before the first chunks.
pub fn pre_spawn_packets(
    player: &Player,
    world: &World,
    tick: u64,
    motd: &str,
    palette: &BlockPalette,
) -> Vec<Vec<u8>> {
    vec![
        jigsaw_structure_data_packet(),
        voxel_shapes_packet(),
        start_game_packet(player, world, tick, motd, palette),
        item_registry_packet(),
        available_actor_identifiers_packet(),
        biome_definition_list_packet(),
    ]
}

/// Convenience for the initial view-distance chunk burst.
pub fn chunk_burst(
    world: &World,
    center: BlockPosition,
    radius: i32,
    palette: &BlockPalette,
) -> Vec<Vec<u8>> {
    let (center_x, center_z) = (center.chunk_x(), center.chunk_z());
    let mut packets = Vec::new();
    for x in (center_x - radius)..=(center_x + radius) {
        for z in (center_z - radius)..=(center_z + radius) {
            world.load_chunk(x, z);
            if let Some(packet) = level_chunk_packet(world, x, z, palette) {
                packets.push(packet);
            }
        }
    }
    packets
}

#[cfg(test)]
mod tests {
    use super::*;
    use pm_math::Vector3;

    fn flat_world() -> World {
        let world = World::flat("test");
        world.load_chunk(0, 0);
        world.load_chunk(1, 0);
        world
    }

    #[test]
    fn chunk_packet_has_expected_header() {
        let world = flat_world();
        let palette = BlockPalette::build(&world);
        let packet = level_chunk_packet(&world, 0, 0, &palette).unwrap();

        let mut offset = 0usize;
        assert_eq!(
            pm_network::read_varint(&packet, &mut offset),
            Some(LEVEL_CHUNK_PACKET)
        );
        assert_eq!(
            pm_network::read_varint_signed(&packet, &mut offset),
            Some(0)
        );
        assert_eq!(
            pm_network::read_varint_signed(&packet, &mut offset),
            Some(0)
        );
        assert_eq!(
            pm_network::read_varint_signed(&packet, &mut offset),
            Some(0)
        ); // dimension
        assert_eq!(pm_network::read_varint(&packet, &mut offset), Some(9)); // sections -4..4
    }

    #[test]
    fn start_game_contains_palette_and_player() {
        let world = flat_world();
        let player = Player::new("Steve", "uuid", Vector3::new(0.5, 4.0, 0.5), "test");
        let palette = BlockPalette::build(&world);
        let packet = start_game_packet(&player, &world, 42, "MOTD", &palette);

        let mut offset = 0usize;
        assert_eq!(
            pm_network::read_varint(&packet, &mut offset),
            Some(START_GAME_PACKET)
        );
        assert!(palette.len() >= 4);
        let haystack = &packet[offset..];
        assert!(haystack.windows(15).any(|w| w == b"minecraft:stone"));
        assert!(haystack.windows(17).any(|w| w == b"minecraft:bedrock"));
    }

    #[test]
    fn item_registry_contains_bedrock_items() {
        let packet = item_registry_packet();
        let mut offset = 0usize;
        assert_eq!(
            pm_network::read_varint(&packet, &mut offset),
            Some(ITEM_REGISTRY_PACKET)
        );
        let count = pm_network::read_varint(&packet, &mut offset).unwrap();
        assert!(count > 1000, "expected the full item registry, got {count}");
        let body = &packet[offset..];
        assert!(body.windows(17).any(|w| w == b"minecraft:diamond"));
    }

    #[test]
    fn palette_scans_placed_blocks() {
        let world = flat_world();
        world.set_block_at(3, 10, 3, pm_world::blocks::stone());
        world.set_block_at(4, 10, 4, Block::of("minecraft:diamond_block"));
        let palette = BlockPalette::build(&world);
        assert!(palette
            .entries
            .iter()
            .any(|(name, _)| name == "minecraft:diamond_block"));
        assert!(palette.id(&Block::of("minecraft:diamond_block")) > 0);
        assert_eq!(palette.id(&Block::air()), 0);
    }

    #[test]
    fn sub_chunk_packing_roundtrips_simple_palette() {
        let world = flat_world();
        let palette = BlockPalette::build(&world);
        let payload = world
            .with_chunk(0, 0, |chunk| serialize_full_chunk(chunk, &palette))
            .unwrap();

        // First section is at y=-64 and empty: version 8, 1 layer, bits 0.
        assert_eq!(payload[0], 8);
        assert_eq!(payload[1], 1);
        assert_eq!(payload[2], 1);
    }
}
