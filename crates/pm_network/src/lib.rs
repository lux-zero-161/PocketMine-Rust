//! RakNet / Bedrock network layer.
//!
//! Implemented milestones:
//! * Unconnected ping/pong, so the server appears in the Minecraft LAN list.
//! * RakNet offline handshake (request 1/2, connection request, new incoming
//!   connection) with MTU negotiation and ACKs.
//! * Datagram + encapsulated frame codec, including **split frames** for
//!   payloads larger than the MTU.
//! * Bedrock packet batches (`varint length + packet`), raw-deflate
//!   compression with the protocol 1.20.60+ compression-type prefix.
//! * `RequestNetworkSettings` -> `NetworkSettings` (compression negotiation).
//! * Offline-mode `Login` inspection (identity from the JWT chain) and a clean
//!   `PlayStatus` rejection until the encrypted login milestone is complete.

pub mod encryption;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use encryption::{create_handshake_jwt, derive_aes_key, derive_shared_secret, random_salt, EncryptionContext};
use openssl::pkey::{PKey, Private};
use pm_core::{Logger, RAKNET_MAGIC};
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub mod protocol {
    //! Bedrock packet identifiers (subset, grows as handlers are ported).
    pub const LOGIN: u8 = 0x01;
    pub const PLAY_STATUS: u8 = 0x02;
    pub const SERVER_TO_CLIENT_HANDSHAKE: u8 = 0x03;
    pub const CLIENT_TO_SERVER_HANDSHAKE: u8 = 0x04;
    pub const DISCONNECT: u8 = 0x05;
    pub const RESOURCE_PACK_CLIENT_RESPONSE: u8 = 0x08;
    pub const TEXT: u8 = 0x09;
    pub const SET_TIME: u8 = 0x0a;
    pub const START_GAME: u8 = 0x0b;
    pub const ADD_PLAYER: u8 = 0x0c;
    pub const ADD_ENTITY: u8 = 0x0d;
    pub const REMOVE_ENTITY: u8 = 0x0e;
    pub const MOVE_PLAYER: u8 = 0x12;
    pub const LEVEL_CHUNK: u8 = 0x3a;
    pub const REQUEST_CHUNK_RADIUS: u8 = 0x45;
    pub const CHUNK_RADIUS_UPDATED: u8 = 0x46;
    pub const REQUEST_NETWORK_SETTINGS: u8 = 0x5a;
    pub const SET_LOCAL_PLAYER_AS_INITIALIZED: u8 = 0x71;
    pub const REQUEST_CAPABILITIES: u8 = 0x77;
    pub const NETWORK_SETTINGS: u8 = 0x8f;

    pub const PLAY_STATUS_LOGIN_SUCCESS: i32 = 0;
    pub const PLAY_STATUS_LOGIN_FAILED_SERVER_FULL: i32 = 1;
    pub const PLAY_STATUS_LOGIN_FAILED_CLIENT_OLD: i32 = 2;
    pub const PLAY_STATUS_LOGIN_FAILED_SERVER_OLD: i32 = 3;
}

use protocol::*;

// ---------------------------------------------------------------------------
// RakNet constants
// ---------------------------------------------------------------------------

const ID_UNCONNECTED_PING: u8 = 0x01;
const ID_UNCONNECTED_PONG: u8 = 0x1c;
const ID_OPEN_CONNECTION_REQUEST_1: u8 = 0x05;
const ID_OPEN_CONNECTION_REPLY_1: u8 = 0x06;
const ID_OPEN_CONNECTION_REQUEST_2: u8 = 0x07;
const ID_OPEN_CONNECTION_REPLY_2: u8 = 0x08;
const ID_CONNECTION_REQUEST: u8 = 0x09;
const ID_CONNECTION_REQUEST_ACCEPTED: u8 = 0x10;
const ID_NEW_INCOMING_CONNECTION: u8 = 0x13;
const ID_DATAGRAM: u8 = 0x84;
const ID_ACK: u8 = 0xc0;
const ID_NACK: u8 = 0xa0;

const RAKNET_PROTOCOL_VERSION: u8 = 11;
const DEFAULT_MTU: u16 = 1400;
const DATAGRAM_HEADER_SIZE: usize = 4;

const COMPRESSION_NONE: u8 = 0xff;
const COMPRESSION_ZLIB: u8 = 0x00;

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn write_address(buf: &mut Vec<u8>, addr: SocketAddr) {
    match addr {
        SocketAddr::V4(v4) => {
            buf.push(4);
            for octet in v4.ip().octets() {
                buf.push(!octet);
            }
            buf.extend_from_slice(&v4.port().to_be_bytes());
        }
        SocketAddr::V6(v6) => {
            buf.push(6);
            buf.extend_from_slice(&[0u8; 8]);
            buf.extend_from_slice(&[0u8; 2]);
            buf.extend_from_slice(&[0u8; 4]);
            buf.extend_from_slice(&v6.ip().octets());
            buf.extend_from_slice(&v6.port().to_be_bytes());
        }
    }
}

fn read_magic(buf: &[u8], offset: usize) -> bool {
    buf.len() >= offset + 16 && &buf[offset..offset + 16] == RAKNET_MAGIC
}

pub fn write_varint(out: &mut Vec<u8>, mut value: u32) {
    loop {
        if value & !0x7f == 0 {
            out.push(value as u8);
            return;
        }
        out.push(((value & 0x7f) | 0x80) as u8);
        value >>= 7;
    }
}

pub fn read_varint(input: &[u8], offset: &mut usize) -> Option<u32> {
    let mut value = 0u32;
    let mut shift = 0;
    loop {
        let byte = *input.get(*offset)?;
        *offset += 1;
        value |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
        if shift >= 35 {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Packet batches and compression
// ---------------------------------------------------------------------------

/// Encodes packets as a Bedrock batch (`varint length` before each packet).
pub fn encode_batch(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packets.iter().map(Vec::len).sum::<usize>() + packets.len());
    for packet in packets {
        write_varint(&mut out, packet.len() as u32);
        out.extend_from_slice(packet);
    }
    out
}

/// Splits a batch back into individual packets.
pub fn decode_batch(batch: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let mut packets = Vec::new();
    let mut offset = 0usize;
    while offset < batch.len() {
        let length = read_varint(batch, &mut offset).ok_or("invalid packet length")? as usize;
        if offset + length > batch.len() {
            return Err("packet length exceeds batch".to_string());
        }
        packets.push(batch[offset..offset + length].to_vec());
        offset += length;
    }
    Ok(packets)
}

/// Applies Bedrock's protocol >= 1.20.60 compression envelope. The first byte
/// is the algorithm (`0` = zlib/raw deflate, `0xff` = none).
pub fn compress_batch(batch: &[u8], threshold: usize) -> Vec<u8> {
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    if batch.len() < threshold {
        let mut out = Vec::with_capacity(batch.len() + 1);
        out.push(COMPRESSION_NONE);
        out.extend_from_slice(batch);
        return out;
    }

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::new(7));
    if encoder.write_all(batch).is_err() {
        let mut out = Vec::with_capacity(batch.len() + 1);
        out.push(COMPRESSION_NONE);
        out.extend_from_slice(batch);
        return out;
    }
    match encoder.finish() {
        Ok(compressed) => {
            let mut out = Vec::with_capacity(compressed.len() + 1);
            out.push(COMPRESSION_ZLIB);
            out.extend_from_slice(&compressed);
            out
        }
        Err(_) => {
            let mut out = Vec::with_capacity(batch.len() + 1);
            out.push(COMPRESSION_NONE);
            out.extend_from_slice(batch);
            out
        }
    }
}

/// Removes the compression envelope and returns the raw batch.
pub fn decompress_batch(payload: &[u8]) -> Result<Vec<u8>, String> {
    use flate2::read::DeflateDecoder;
    use std::io::Read;

    let Some((&kind, body)) = payload.split_first() else {
        return Err("empty compressed payload".to_string());
    };
    match kind {
        COMPRESSION_NONE => Ok(body.to_vec()),
        COMPRESSION_ZLIB => {
            let mut decoder = DeflateDecoder::new(body);
            let mut out = Vec::new();
            decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
            Ok(out)
        }
        other => Err(format!("unsupported compression algorithm {other}")),
    }
}

// ---------------------------------------------------------------------------
// Encapsulated frames
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub reliability: u8,
    pub reliable_index: Option<u32>,
    pub sequence_index: Option<u32>,
    pub order_index: Option<u32>,
    pub order_channel: Option<u8>,
    pub split_count: u32,
    pub split_id: u16,
    pub split_index: u32,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Reliable ordered frame, used for all Bedrock game packets.
    pub fn reliable_ordered(payload: Vec<u8>, reliable_index: u32, order_index: u32) -> Self {
        Self {
            reliability: 3,
            reliable_index: Some(reliable_index),
            sequence_index: None,
            order_index: Some(order_index),
            order_channel: Some(0),
            split_count: 0,
            split_id: 0,
            split_index: 0,
            payload,
        }
    }

    pub fn is_split(&self) -> bool {
        self.split_count > 0
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        let mut flags = 0x80 | (self.reliability << 5);
        if self.is_split() {
            flags |= 0x10;
        }
        out.push(flags);
        out.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());

        let reliable = matches!(self.reliability, 2 | 3 | 4 | 6 | 7);
        let sequenced = matches!(self.reliability, 1 | 4);
        let ordered = matches!(self.reliability, 3 | 7);

        if reliable {
            out.extend_from_slice(&self.reliable_index.unwrap_or(0).to_le_bytes());
        }
        if sequenced {
            out.extend_from_slice(&self.sequence_index.unwrap_or(0).to_le_bytes());
        }
        if ordered {
            out.extend_from_slice(&self.order_index.unwrap_or(0).to_le_bytes());
            out.push(self.order_channel.unwrap_or(0));
        }
        if self.is_split() {
            out.extend_from_slice(&self.split_count.to_be_bytes());
            out.extend_from_slice(&self.split_id.to_be_bytes());
            out.extend_from_slice(&self.split_index.to_be_bytes());
        }
        out.extend_from_slice(&self.payload);
    }
}

fn decode_frames(buf: &[u8]) -> Vec<Frame> {
    let mut frames = Vec::new();
    let mut i = 0usize;
    while i + 3 <= buf.len() {
        let flags = buf[i];
        i += 1;
        let reliability = (flags >> 5) & 0x07;
        let split = flags & 0x10 != 0;
        let length = u16::from_be_bytes([buf[i], buf[i + 1]]) as usize;
        i += 2;

        let reliable = matches!(reliability, 2 | 3 | 4 | 6 | 7);
        let sequenced = matches!(reliability, 1 | 4);
        let ordered = matches!(reliability, 3 | 7);

        let mut reliable_index = None;
        let mut sequence_index = None;
        let mut order_index = None;
        let mut order_channel = None;

        if reliable && i + 4 <= buf.len() {
            reliable_index = Some(u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]));
            i += 4;
        }
        if sequenced && i + 4 <= buf.len() {
            sequence_index = Some(u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]));
            i += 4;
        }
        if ordered && i + 5 <= buf.len() {
            order_index = Some(u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]));
            order_channel = Some(buf[i + 4]);
            i += 5;
        }

        let mut split_count = 0;
        let mut split_id = 0;
        let mut split_index = 0;
        if split && i + 10 <= buf.len() {
            split_count = u32::from_be_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
            split_id = u16::from_be_bytes([buf[i + 4], buf[i + 5]]);
            split_index = u32::from_be_bytes([buf[i + 6], buf[i + 7], buf[i + 8], buf[i + 9]]);
            i += 10;
        }

        if i + length > buf.len() {
            break;
        }
        frames.push(Frame {
            reliability,
            reliable_index,
            sequence_index,
            order_index,
            order_channel,
            split_count,
            split_id,
            split_index,
            payload: buf[i..i + length].to_vec(),
        });
        i += length;
    }
    frames
}

fn encode_datagram(sequence: u32, frames: &[Frame]) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.push(ID_DATAGRAM);
    out.extend_from_slice(&(sequence & 0x00ff_ffff).to_be_bytes()[1..]);
    for frame in frames {
        frame.encode(&mut out);
    }
    out
}

fn encode_ack(sequences: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + sequences.len() * 3);
    out.push(ID_ACK);
    out.extend_from_slice(&(sequences.len() as u16).to_be_bytes());
    for seq in sequences {
        out.extend_from_slice(&(seq & 0x00ff_ffff).to_be_bytes()[1..]);
    }
    out
}

/// Reassembles split frames for one session.
#[derive(Debug, Default)]
pub struct SplitAssembly {
    count: u32,
    received: u32,
    parts: Vec<Option<Vec<u8>>>,
}

impl SplitAssembly {
    pub fn new(count: u32) -> Self {
        Self { count, received: 0, parts: vec![None; count as usize] }
    }

    /// Returns the complete payload once every fragment has arrived.
    pub fn push(&mut self, index: u32, payload: Vec<u8>) -> Option<Vec<u8>> {
        let index = index as usize;
        if index >= self.parts.len() || self.parts[index].is_some() {
            return None;
        }
        self.parts[index] = Some(payload);
        self.received += 1;
        if self.received < self.count {
            return None;
        }
        let mut out = Vec::new();
        for part in self.parts.iter() {
            out.extend_from_slice(part.as_ref()?);
        }
        Some(out)
    }
}

/// Splits a payload into frames small enough for the given MTU.
pub fn split_payload(payload: &[u8], mtu: usize, reliable_start: u32, order_start: u32) -> Vec<Frame> {
    let max = mtu.saturating_sub(DATAGRAM_HEADER_SIZE + 3 + 10 + 5 + 32).max(256);
    if payload.len() <= max {
        return vec![Frame::reliable_ordered(payload.to_vec(), reliable_start, order_start)];
    }
    let count = payload.len().div_ceil(max);
    let split_id = (reliable_start & 0xffff) as u16;
    let mut frames = Vec::with_capacity(count);
    for (index, chunk) in payload.chunks(max).enumerate() {
        let mut frame = Frame::reliable_ordered(
            chunk.to_vec(),
            reliable_start.wrapping_add(index as u32),
            order_start.wrapping_add(index as u32),
        );
        frame.split_count = count as u32;
        frame.split_id = split_id;
        frame.split_index = index as u32;
        frames.push(frame);
    }
    frames
}

// ---------------------------------------------------------------------------
// Pong data / sessions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PongData {
    pub motd: String,
    pub sub_motd: String,
    pub protocol: i32,
    pub version: String,
    pub online: u32,
    pub max: u32,
    pub server_guid: i64,
    pub gamemode: String,
    pub port_v4: u16,
    pub port_v6: u16,
}

impl PongData {
    pub fn encode(&self) -> String {
        format!(
            "MCPE;{};{};{};{};{};{};{};{};{};{};{};",
            self.motd,
            self.protocol,
            self.version,
            self.online,
            self.max,
            self.server_guid,
            self.sub_motd,
            self.gamemode,
            gamemode_id(&self.gamemode),
            self.port_v4,
            self.port_v6
        )
    }
}

fn gamemode_id(name: &str) -> i32 {
    match name.to_ascii_lowercase().as_str() {
        "creative" => 1,
        "adventure" => 2,
        "spectator" => 6,
        _ => 0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Negotiating,
    Connected,
}

pub struct Session {
    pub address: SocketAddr,
    pub mtu: u16,
    pub state: SessionState,
    pub datagram_sequence: u32,
    pub reliable_index: u32,
    pub order_index: u32,
    pub last_seen: i64,
    pub compression_threshold: u16,
    pub compression_enabled: bool,
    pub encryption: Option<EncryptionContext>,
    split_assemblies: HashMap<u16, SplitAssembly>,
}

impl Session {
    fn new(address: SocketAddr, mtu: u16) -> Self {
        Self {
            address,
            mtu,
            state: SessionState::Negotiating,
            datagram_sequence: 0,
            reliable_index: 0,
            order_index: 0,
            last_seen: now_millis(),
            compression_threshold: 256,
            compression_enabled: false,
            encryption: None,
            split_assemblies: HashMap::new(),
        }
    }

    fn next_datagram_sequence(&mut self) -> u32 {
        let seq = self.datagram_sequence;
        self.datagram_sequence = self.datagram_sequence.wrapping_add(1) & 0x00ff_ffff;
        seq
    }

}

/// Identity extracted from an offline-mode Login packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginIdentity {
    pub protocol: u32,
    pub display_name: String,
    pub identity: String,
    pub xuid: String,
    /// Client ECDH public key (DER/SPKI), required for the handshake.
    pub client_public_key: Option<Vec<u8>>,
}

#[derive(Debug, Default, Clone)]
pub struct NetworkStats {
    pub pings: u64,
    pub pongs: u64,
    pub handshakes: u64,
    pub datagrams_in: u64,
    pub datagrams_out: u64,
    pub frames_in: u64,
    pub batches_in: u64,
    pub logins: u64,
    pub encryptions: u64,
    pub unknown_packets: u64,
}

// ---------------------------------------------------------------------------
// Server
// ---------------------------------------------------------------------------

pub struct BedrockNetworkServer {
    socket: Arc<UdpSocket>,
    running: Arc<AtomicBool>,
    logger: Arc<Logger>,
    pong: Arc<RwLock<PongData>>,
    sessions: Arc<RwLock<HashMap<SocketAddr, Session>>>,
    stats: Arc<RwLock<NetworkStats>>,
    packet_handler: Arc<RwLock<Option<Box<dyn FnMut(SocketAddr, &[u8]) + Send + Sync>>>>,
    next_guid: AtomicU64,
    server_key: Arc<PKey<Private>>,
}

impl BedrockNetworkServer {
    pub fn bind(bind_addr: &str, motd: &str, max_players: usize, logger: Arc<Logger>) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        let local = socket.local_addr()?;

        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let guid = (now.as_nanos() as i64).wrapping_mul(31) | 1;
        let server_key = Arc::new(
            encryption::generate_server_key()
                .map_err(|err| io::Error::new(io::ErrorKind::Other, format!("ECDH key generation failed: {err}")))?,
        );

        let pong = PongData {
            motd: motd.to_string(),
            sub_motd: "§bRust core · C# plugins".to_string(),
            protocol: pm_core::CURRENT_PROTOCOL,
            version: pm_core::MINECRAFT_VERSION_NETWORK.to_string(),
            online: 0,
            max: max_players as u32,
            server_guid: guid,
            gamemode: "Survival".to_string(),
            port_v4: local.port(),
            port_v6: local.port().saturating_add(1),
        };

        logger.info(&format!(
            "RakNet listener ready on udp://{} (protocol {}, server id {})",
            local,
            pm_core::CURRENT_PROTOCOL,
            guid
        ));

        Ok(Self {
            socket: Arc::new(socket),
            running: Arc::new(AtomicBool::new(true)),
            logger,
            pong: Arc::new(RwLock::new(pong)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(NetworkStats::default())),
            packet_handler: Arc::new(RwLock::new(None)),
            next_guid: AtomicU64::new(guid as u64),
            server_key,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn set_online_count(&self, online: u32) {
        self.pong.write().unwrap().online = online;
    }

    pub fn set_motd(&self, motd: &str) {
        self.pong.write().unwrap().motd = motd.to_string();
    }

    pub fn stats(&self) -> NetworkStats {
        self.stats.read().unwrap().clone()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.read().unwrap().len()
    }

    pub fn set_packet_handler<F>(&self, handler: F)
    where
        F: FnMut(SocketAddr, &[u8]) + Send + Sync + 'static,
    {
        *self.packet_handler.write().unwrap() = Some(Box::new(handler));
    }

    /// Drains all pending datagrams. Called once per server tick.
    pub fn process_packets(&self) {
        if !self.running.load(Ordering::Relaxed) {
            return;
        }
        let mut buf = [0u8; 2048];
        loop {
            match self.socket.recv_from(&mut buf) {
                Ok((len, addr)) => self.handle_datagram(addr, &buf[..len]),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    self.logger.warning(&format!("UDP receive error: {}", e));
                    break;
                }
            }
        }
    }

    fn handle_datagram(&self, addr: SocketAddr, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        match data[0] {
            ID_UNCONNECTED_PING => self.handle_ping(addr, data),
            ID_OPEN_CONNECTION_REQUEST_1 => self.handle_open_request_1(addr, data),
            ID_OPEN_CONNECTION_REQUEST_2 => self.handle_open_request_2(addr, data),
            ID_CONNECTION_REQUEST => self.handle_connection_request(addr, data),
            ID_NEW_INCOMING_CONNECTION => self.handle_new_incoming(addr, data),
            ID_ACK | ID_NACK => {}
            ID_DATAGRAM => self.handle_datagram_frame(addr, data),
            _ => {
                self.stats.write().unwrap().unknown_packets += 1;
                self.logger.debug(&format!(
                    "Unhandled RakNet datagram id 0x{:02x} ({} bytes) from {}",
                    data[0],
                    data.len(),
                    addr
                ));
            }
        }
    }

    fn handle_ping(&self, addr: SocketAddr, data: &[u8]) {
        if data.len() < 33 || !read_magic(data, 9) {
            return;
        }
        let ping_time = i64::from_be_bytes(data[1..9].try_into().unwrap());
        self.stats.write().unwrap().pings += 1;

        let (pong_text, guid) = {
            let pong = self.pong.read().unwrap();
            (pong.encode(), pong.server_guid as u64)
        };
        let mut out = Vec::with_capacity(35 + pong_text.len());
        out.push(ID_UNCONNECTED_PONG);
        out.extend_from_slice(&ping_time.to_be_bytes());
        out.extend_from_slice(&guid.to_be_bytes());
        out.extend_from_slice(&RAKNET_MAGIC);
        out.extend_from_slice(&(pong_text.len() as u16).to_be_bytes());
        out.extend_from_slice(pong_text.as_bytes());
        let _ = self.socket.send_to(&out, addr);
        self.stats.write().unwrap().pongs += 1;
    }

    fn handle_open_request_1(&self, addr: SocketAddr, data: &[u8]) {
        if data.len() < 18 || !read_magic(data, 1) {
            return;
        }
        let client_protocol = data[17];
        if client_protocol != RAKNET_PROTOCOL_VERSION {
            self.logger.debug(&format!(
                "RakNet protocol {} from {} (server uses {})",
                client_protocol, addr, RAKNET_PROTOCOL_VERSION
            ));
        }
        let mtu = (data.len() as u16 + 28).clamp(576, 1492);
        let mut out = Vec::with_capacity(28);
        out.push(ID_OPEN_CONNECTION_REPLY_1);
        out.extend_from_slice(&RAKNET_MAGIC);
        out.extend_from_slice(&self.next_guid.load(Ordering::Relaxed).to_be_bytes());
        out.push(0);
        out.extend_from_slice(&mtu.to_be_bytes());
        let _ = self.socket.send_to(&out, addr);
        self.stats.write().unwrap().handshakes += 1;
    }

    fn handle_open_request_2(&self, addr: SocketAddr, data: &[u8]) {
        if data.len() < 19 || !read_magic(data, 1) {
            return;
        }
        let mtu = if data.len() >= 10 {
            u16::from_be_bytes([data[data.len() - 10], data[data.len() - 9]])
        } else {
            DEFAULT_MTU
        };
        let mtu = mtu.clamp(576, 1492);
        let mut out = Vec::with_capacity(32);
        out.push(ID_OPEN_CONNECTION_REPLY_2);
        out.extend_from_slice(&RAKNET_MAGIC);
        out.extend_from_slice(&self.next_guid.load(Ordering::Relaxed).to_be_bytes());
        write_address(&mut out, addr);
        out.extend_from_slice(&mtu.to_be_bytes());
        out.push(0);
        let _ = self.socket.send_to(&out, addr);

        self.sessions.write().unwrap().insert(addr, Session::new(addr, mtu));
        self.stats.write().unwrap().handshakes += 1;
    }

    fn handle_connection_request(&self, addr: SocketAddr, data: &[u8]) {
        if data.len() < 18 {
            return;
        }
        let time = i64::from_be_bytes(data[9..17].try_into().unwrap());
        let mut out = Vec::with_capacity(128);
        out.push(ID_CONNECTION_REQUEST_ACCEPTED);
        write_address(&mut out, addr);
        out.extend_from_slice(&0u16.to_be_bytes());
        for _ in 0..10 {
            write_address(&mut out, addr);
        }
        out.extend_from_slice(&time.to_be_bytes());
        out.extend_from_slice(&now_millis().to_be_bytes());
        let _ = self.socket.send_to(&out, addr);
    }

    fn handle_new_incoming(&self, addr: SocketAddr, _data: &[u8]) {
        if let Some(session) = self.sessions.write().unwrap().get_mut(&addr) {
            session.state = SessionState::Connected;
        }
        self.logger.debug(&format!("RakNet client connected: {}", addr));
    }

    fn handle_datagram_frame(&self, addr: SocketAddr, data: &[u8]) {
        if data.len() < 4 {
            return;
        }
        let sequence = u32::from_be_bytes([0, data[1], data[2], data[3]]);
        let frames = decode_frames(&data[4..]);

        let compression_enabled = {
            let sessions = self.sessions.read().unwrap();
            sessions.get(&addr).map(|s| s.compression_enabled).unwrap_or(false)
        };

        {
            let mut stats = self.stats.write().unwrap();
            stats.datagrams_in += 1;
            stats.frames_in += frames.len() as u64;
        }

        for frame in frames {
            let payload = if frame.is_split() {
                let mut sessions = self.sessions.write().unwrap();
                let Some(session) = sessions.get_mut(&addr) else { continue };
                let assembly = session
                    .split_assemblies
                    .entry(frame.split_id)
                    .or_insert_with(|| SplitAssembly::new(frame.split_count));
                let complete = assembly.push(frame.split_index, frame.payload);
                if complete.is_some() {
                    session.split_assemblies.remove(&frame.split_id);
                }
                complete
            } else {
                Some(frame.payload)
            };

            if let Some(payload) = payload {
                self.handle_payload(addr, &payload, compression_enabled);
            }
        }

        let ack = encode_ack(&[sequence]);
        let _ = self.socket.send_to(&ack, addr);
    }

    fn handle_payload(&self, addr: SocketAddr, payload: &[u8], compression_enabled: bool) {
        let encrypted = {
            let sessions = self.sessions.read().unwrap();
            sessions.get(&addr).map(|s| s.encryption.is_some()).unwrap_or(false)
        };
        let decrypted;
        let payload = if encrypted {
            let mut sessions = self.sessions.write().unwrap();
            let Some(session) = sessions.get_mut(&addr) else { return };
            let Some(context) = session.encryption.as_mut() else { return };
            match context.decrypt(payload) {
                Ok(data) => {
                    decrypted = data;
                    decrypted.as_slice()
                }
                Err(err) => {
                    self.logger.debug(&format!("Decryption failed for {}: {}", addr, err));
                    self.disconnect(addr);
                    return;
                }
            }
        } else {
            payload
        };

        let batch = if compression_enabled {
            match decompress_batch(payload) {
                Ok(batch) => batch,
                Err(err) => {
                    self.logger.debug(&format!("Bad compressed batch from {}: {}", addr, err));
                    return;
                }
            }
        } else {
            payload.to_vec()
        };

        let packets = match decode_batch(&batch) {
            Ok(packets) => packets,
            Err(err) => {
                self.logger.debug(&format!("Bad packet batch from {}: {}", addr, err));
                return;
            }
        };
        self.stats.write().unwrap().batches_in += 1;

        for packet in packets {
            if packet.is_empty() {
                continue;
            }
            self.handle_packet(addr, &packet);
        }
    }

    fn handle_packet(&self, addr: SocketAddr, packet: &[u8]) {
        match packet[0] {
            REQUEST_NETWORK_SETTINGS => self.send_network_settings(addr),
            LOGIN => {
                self.stats.write().unwrap().logins += 1;
                match parse_login(packet) {
                    Some(identity) => {
                        self.logger.info(&format!(
                            "Login attempt from {}: \"{}\" ({}) protocol {}",
                            addr, identity.display_name, identity.identity, identity.protocol
                        ));
                        if identity.protocol != pm_core::CURRENT_PROTOCOL as u32 {
                            self.logger.warning(&format!(
                                "Client protocol {} is not supported (server: {})",
                                identity.protocol,
                                pm_core::CURRENT_PROTOCOL
                            ));
                            self.send_play_status(addr, PLAY_STATUS_LOGIN_FAILED_CLIENT_OLD);
                            return;
                        }
                        if !self.begin_login_encryption(addr, &identity) {
                            self.send_play_status(addr, PLAY_STATUS_LOGIN_FAILED_SERVER_OLD);
                        }
                    }
                    None => {
                        self.logger.debug(&format!("Could not parse Login packet from {}", addr));
                        self.send_play_status(addr, PLAY_STATUS_LOGIN_FAILED_SERVER_OLD);
                    }
                }
            }
            CLIENT_TO_SERVER_HANDSHAKE => {
                self.stats.write().unwrap().encryptions += 1;
                self.logger.info(&format!("Encryption handshake completed for {}", addr));
                self.send_play_status(addr, PLAY_STATUS_LOGIN_SUCCESS);
                // StartGame/chunk streaming is the next milestone. Disconnect
                // with a readable reason instead of leaving the client hanging.
                self.send_disconnect(addr, "PocketMine-Rust: encryption OK, world streaming is the next milestone");
            }
            _ => {
                if let Some(handler) = self.packet_handler.write().unwrap().as_mut() {
                    handler(addr, packet);
                }
            }
        }
    }

    fn send_network_settings(&self, addr: SocketAddr) {
        let mut payload = Vec::with_capacity(7);
        payload.push(NETWORK_SETTINGS);
        payload.extend_from_slice(&256u16.to_le_bytes()); // threshold
        payload.extend_from_slice(&0u16.to_le_bytes()); // zlib
        payload.extend_from_slice(&256u16.to_le_bytes()); // client threshold

        // NetworkSettings is the last uncompressed batch.
        self.send_packets(addr, vec![payload], Some(false));

        if let Some(session) = self.sessions.write().unwrap().get_mut(&addr) {
            session.compression_enabled = true;
            session.compression_threshold = 256;
        }
    }

    fn send_play_status(&self, addr: SocketAddr, status: i32) {
        let mut payload = Vec::with_capacity(5);
        payload.push(PLAY_STATUS);
        payload.extend_from_slice(&status.to_be_bytes());
        self.send_packets(addr, vec![payload], None);
    }

    fn send_disconnect(&self, addr: SocketAddr, message: &str) {
        let mut payload = Vec::new();
        payload.push(DISCONNECT);
        write_varint(&mut payload, 0); // reason: unknown
        write_varint(&mut payload, 0); // type: message present
        write_varint(&mut payload, message.len() as u32);
        payload.extend_from_slice(message.as_bytes());
        write_varint(&mut payload, 0); // filtered message
        self.send_packets(addr, vec![payload], None);
    }

    /// Performs the `ServerToClientHandshake` exchange. The handshake packet is
    /// sent compressed but still unencrypted; encryption starts right after.
    fn begin_login_encryption(&self, addr: SocketAddr, identity: &LoginIdentity) -> bool {
        let Some(client_der) = identity.client_public_key.as_deref() else {
            self.logger.warning(&format!(
                "Login from {} has no client public key (Xbox-authenticated chains are not supported yet)",
                addr
            ));
            return false;
        };
        let shared_secret = match derive_shared_secret(&self.server_key, client_der) {
            Ok(secret) => secret,
            Err(err) => {
                self.logger.warning(&format!("ECDH with {} failed: {}", addr, err));
                return false;
            }
        };
        let salt = random_salt();
        let key = derive_aes_key(&shared_secret, &salt);
        let jwt = match create_handshake_jwt(&self.server_key, &salt) {
            Ok(jwt) => jwt,
            Err(err) => {
                self.logger.error(&format!("Could not create handshake JWT: {}", err));
                return false;
            }
        };

        let mut packet = Vec::with_capacity(jwt.len() + 6);
        packet.push(SERVER_TO_CLIENT_HANDSHAKE);
        write_varint(&mut packet, jwt.len() as u32);
        packet.extend_from_slice(jwt.as_bytes());
        self.send_packets(addr, vec![packet], None);

        let context = match EncryptionContext::new(key) {
            Ok(context) => context,
            Err(err) => {
                self.logger.error(&format!("Could not enable encryption: {}", err));
                return false;
            }
        };
        let mut sessions = self.sessions.write().unwrap();
        let Some(session) = sessions.get_mut(&addr) else { return false };
        session.encryption = Some(context);
        drop(sessions);
        self.logger.info(&format!("ServerToClientHandshake sent to {}; encryption enabled", addr));
        true
    }

    /// Sends game packets as a Bedrock batch, split across RakNet frames when
    /// the payload exceeds the negotiated MTU.
    pub fn send_packets(&self, addr: SocketAddr, packets: Vec<Vec<u8>>, compress_override: Option<bool>) {
        let mut datagrams = Vec::new();
        {
            let mut sessions = self.sessions.write().unwrap();
            let Some(session) = sessions.get_mut(&addr) else { return };

            let batch = encode_batch(&packets);
            let compress = compress_override.unwrap_or(session.compression_enabled);
            let mut payload = if compress {
                compress_batch(&batch, session.compression_threshold as usize)
            } else {
                batch
            };
            if let Some(context) = session.encryption.as_mut() {
                payload = context.encrypt(&payload);
            }

            let mtu = session.mtu as usize;
            let frames = split_payload(&payload, mtu, session.reliable_index, session.order_index);
            session.reliable_index = session.reliable_index.wrapping_add(frames.len() as u32);
            session.order_index = session.order_index.wrapping_add(frames.len() as u32);

            let mut current: Vec<Frame> = Vec::new();
            let mut current_size = DATAGRAM_HEADER_SIZE;
            for frame in frames {
                let mut encoded = Vec::new();
                frame.encode(&mut encoded);
                if current_size + encoded.len() > mtu && !current.is_empty() {
                    let sequence = session.next_datagram_sequence();
                    datagrams.push(encode_datagram(sequence, &current));
                    current.clear();
                    current_size = DATAGRAM_HEADER_SIZE;
                }
                current_size += encoded.len();
                current.push(frame);
            }
            if !current.is_empty() {
                let sequence = session.next_datagram_sequence();
                datagrams.push(encode_datagram(sequence, &current));
            }
        }

        for datagram in datagrams {
            let _ = self.socket.send_to(&datagram, addr);
            self.stats.write().unwrap().datagrams_out += 1;
        }
    }

    pub fn disconnect(&self, addr: SocketAddr) {
        self.sessions.write().unwrap().remove(&addr);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.sessions.write().unwrap().clear();
    }
}

impl Drop for BedrockNetworkServer {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Login parsing (offline mode)
// ---------------------------------------------------------------------------

fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4 + 3);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => continue,
            _ => return None,
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

fn jwt_part(token: &str, index: usize) -> Option<Value> {
    let part = token.split('.').nth(index)?;
    let data = base64url_decode(part)?;
    serde_json::from_slice(&data).ok()
}

fn jwt_header(token: &str) -> Option<Value> {
    jwt_part(token, 0)
}

fn jwt_payload(token: &str) -> Option<Value> {
    jwt_part(token, 1)
}

/// Parses a Login packet and extracts the player identity plus the client
/// ECDH public key. Signatures are not verified yet (offline mode); the key is
/// what the encrypted handshake needs.
pub fn parse_login(packet: &[u8]) -> Option<LoginIdentity> {
    if packet.len() < 7 || packet[0] != LOGIN {
        return None;
    }
    let protocol = u32::from_be_bytes([packet[1], packet[2], packet[3], packet[4]]);
    let mut offset = 5usize;
    let request_len = read_varint(packet, &mut offset)? as usize;
    if offset + request_len > packet.len() {
        return None;
    }
    let request = &packet[offset..offset + request_len];
    if request.len() < 4 {
        return None;
    }

    let auth_len = u32::from_le_bytes([request[0], request[1], request[2], request[3]]) as usize;
    let auth_end = 4usize.checked_add(auth_len)?;
    if auth_end > request.len() {
        return None;
    }
    let auth_info: Value = serde_json::from_slice(&request[4..auth_end]).ok()?;

    let mut display_name = String::new();
    let mut identity = String::new();
    let mut xuid = String::new();
    let mut client_public_key = None;

    // Protocol >= 1.21.93: {"AuthenticationType":..,"Token":"<jwt>","Certificate":..}
    if let Some(token) = auth_info.get("Token").and_then(Value::as_str) {
        if let Some(claims) = jwt_payload(token) {
            display_name = claims.get("xname").and_then(Value::as_str).unwrap_or("").to_string();
            identity = claims.get("leguuid").and_then(Value::as_str).unwrap_or("").to_string();
            xuid = claims.get("xuid").and_then(Value::as_str).unwrap_or("").to_string();
            if let Some(cpk) = claims.get("cpk").and_then(Value::as_str) {
                client_public_key = STANDARD.decode(cpk).ok();
            }
        }
    }

    // Legacy chain format: {"chain":["<jwt>", ...]}
    if let Some(chain) = auth_info.get("chain").and_then(Value::as_array) {
        for token in chain {
            let Some(token) = token.as_str() else { continue };
            if let Some(claims) = jwt_payload(token) {
                if let Some(extra) = claims.get("extraData") {
                    if display_name.is_empty() {
                        display_name = extra.get("displayName").and_then(Value::as_str).unwrap_or("").to_string();
                    }
                    if xuid.is_empty() {
                        xuid = extra.get("XUID").and_then(Value::as_str).unwrap_or("").to_string();
                    }
                }
                if identity.is_empty() {
                    identity = claims.get("identity").and_then(Value::as_str).unwrap_or("").to_string();
                }
            }
            if client_public_key.is_none() {
                if let Some(header) = jwt_header(token) {
                    if let Some(x5u) = header.get("x5u").and_then(Value::as_str) {
                        client_public_key = STANDARD.decode(x5u).ok();
                    }
                }
            }
        }
    }

    // Client data JWT follows the auth info blob.
    let client_data_offset = auth_end;
    if client_data_offset + 4 <= request.len() {
        let len = u32::from_le_bytes([
            request[client_data_offset],
            request[client_data_offset + 1],
            request[client_data_offset + 2],
            request[client_data_offset + 3],
        ]) as usize;
        let start = client_data_offset + 4;
        if start + len <= request.len() {
            if let Ok(text) = std::str::from_utf8(&request[start..start + len]) {
                if let Some(claims) = jwt_payload(text) {
                    if display_name.is_empty() {
                        display_name = claims
                            .get("ThirdPartyName")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string();
                    }
                }
            }
        }
    }

    if display_name.is_empty() && identity.is_empty() {
        return None;
    }
    Some(LoginIdentity { protocol, display_name, identity, xuid, client_public_key })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encryption::{self, EncryptionContext};
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    use openssl::ec::{EcGroup, EcKey};
    use openssl::nid::Nid;
    use openssl::pkey::PKey;

    fn make_jwt(claims: Value) -> String {
        let header = base64url_encode(b"{\"alg\":\"ES384\"}");
        let body = base64url_encode(claims.to_string().as_bytes());
        format!("{header}.{body}.")
    }

    fn build_auth_login(client_der: &[u8]) -> Vec<u8> {
        let auth_token = make_jwt(serde_json::json!({
            "xname": "Steve",
            "leguuid": "00000000-0000-0000-0000-000000000001",
            "cpk": STANDARD.encode(client_der),
        }));
        let auth_info = serde_json::json!({
            "AuthenticationType": 2,
            "Token": auth_token,
            "Certificate": "",
        })
        .to_string();
        let client_data = make_jwt(serde_json::json!({
            "ThirdPartyName": "Steve",
            "GameVersion": "1.26.60",
        }));

        let mut request = Vec::new();
        request.extend_from_slice(&(auth_info.len() as u32).to_le_bytes());
        request.extend_from_slice(auth_info.as_bytes());
        request.extend_from_slice(&(client_data.len() as u32).to_le_bytes());
        request.extend_from_slice(client_data.as_bytes());

        let mut packet = vec![LOGIN];
        packet.extend_from_slice(&(pm_core::CURRENT_PROTOCOL as u32).to_be_bytes());
        write_varint(&mut packet, request.len() as u32);
        packet.extend_from_slice(&request);
        packet
    }

    #[test]
    fn pong_string_is_parsable() {
        let pong = PongData {
            motd: "Hello".into(),
            sub_motd: "Sub".into(),
            protocol: 2225,
            version: "1.26.60".into(),
            online: 2,
            max: 20,
            server_guid: 42,
            gamemode: "Survival".into(),
            port_v4: 19132,
            port_v6: 19133,
        };
        let encoded = pong.encode();
        let parts: Vec<&str> = encoded.trim_end_matches(';').split(';').collect();
        assert_eq!(parts[0], "MCPE");
        assert_eq!(parts[1], "Hello");
        assert_eq!(parts[2], "2225");
        assert_eq!(parts[4], "2");
        assert_eq!(parts[5], "20");
    }

    #[test]
    fn frame_roundtrip() {
        let frame = Frame::reliable_ordered(vec![0x8f, 1, 2, 3], 7, 9);
        let mut encoded = Vec::new();
        frame.encode(&mut encoded);
        let decoded = decode_frames(&encoded);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].payload, vec![0x8f, 1, 2, 3]);
        assert_eq!(decoded[0].reliable_index, Some(7));
        assert_eq!(decoded[0].order_index, Some(9));

        let datagram = encode_datagram(5, &decoded);
        assert_eq!(datagram[0], ID_DATAGRAM);
        assert_eq!(decode_frames(&datagram[4..])[0].payload.len(), 4);
    }

    #[test]
    fn split_frames_reassemble() {
        let payload: Vec<u8> = (0..3000).map(|i| (i % 251) as u8).collect();
        let frames = split_payload(&payload, 400, 1, 1);
        assert!(frames.len() > 1);
        assert!(frames.iter().all(|f| f.is_split()));

        let mut encoded = Vec::new();
        for frame in &frames {
            frame.encode(&mut encoded);
        }
        let decoded = decode_frames(&encoded);
        assert_eq!(decoded.len(), frames.len());

        let mut assembly = SplitAssembly::new(decoded[0].split_count);
        let mut complete = None;
        for frame in decoded {
            if let Some(result) = assembly.push(frame.split_index, frame.payload) {
                complete = Some(result);
            }
        }
        assert_eq!(complete, Some(payload));
    }

    #[test]
    fn batch_roundtrip() {
        let packets = vec![vec![0x01, 2, 3], vec![0xff; 300]];
        let batch = encode_batch(&packets);
        assert_eq!(decode_batch(&batch).unwrap(), packets);
    }

    #[test]
    fn compression_roundtrip_and_threshold() {
        let small = vec![0x09, 1, 2, 3];
        let compressed = compress_batch(&small, 256);
        assert_eq!(compressed[0], COMPRESSION_NONE);
        assert_eq!(decompress_batch(&compressed).unwrap(), small);

        let large = vec![0x3a; 4096];
        let compressed = compress_batch(&large, 256);
        assert_eq!(compressed[0], COMPRESSION_ZLIB);
        assert!(compressed.len() < large.len());
        assert_eq!(decompress_batch(&compressed).unwrap(), large);
    }

    fn build_login_packet() -> Vec<u8> {
        let payload = serde_json::json!({
            "identity": "00000000-0000-0000-0000-000000000001",
            "extraData": { "displayName": "Steve", "XUID": "" }
        })
        .to_string();
        let header = base64url_encode(b"{\"alg\":\"none\"}");
        let body = base64url_encode(payload.as_bytes());
        let token = format!("{header}.{body}.");
        let chain = format!("{{\"chain\":[\"{token}\"]}}");

        let mut request = Vec::new();
        request.extend_from_slice(&(chain.len() as u32).to_le_bytes());
        request.extend_from_slice(chain.as_bytes());
        request.extend_from_slice(&0u32.to_le_bytes());

        let mut packet = vec![LOGIN];
        packet.extend_from_slice(&2225u32.to_be_bytes());
        write_varint(&mut packet, request.len() as u32);
        packet.extend_from_slice(&request);
        packet
    }

    #[test]
    fn parses_offline_login_identity() {
        let packet = build_login_packet();
        let identity = parse_login(&packet).expect("login must parse");
        assert_eq!(identity.display_name, "Steve");
        assert_eq!(identity.identity, "00000000-0000-0000-0000-000000000001");
        assert_eq!(identity.protocol, 2225);
    }

    fn base64url_encode(input: &[u8]) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        let mut buffer = 0u32;
        let mut bits = 0u32;
        for byte in input {
            buffer = (buffer << 8) | *byte as u32;
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                out.push(ALPHABET[((buffer >> bits) & 0x3f) as usize] as char);
            }
        }
        if bits > 0 {
            out.push(ALPHABET[((buffer << (6 - bits)) & 0x3f) as usize] as char);
        }
        out
    }

    fn raknet_handshake(client: &UdpSocket, server: &BedrockNetworkServer) {
        let addr = server.local_addr().unwrap();

        let mut request1 = vec![ID_OPEN_CONNECTION_REQUEST_1];
        request1.extend_from_slice(&RAKNET_MAGIC);
        request1.push(RAKNET_PROTOCOL_VERSION);
        request1.extend_from_slice(&[0u8; 1300]);
        client.send_to(&request1, addr).unwrap();
        server.process_packets();
        let mut buf = [0u8; 2048];
        let (len, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(buf[0], ID_OPEN_CONNECTION_REPLY_1, "len={len}");
        let mtu = u16::from_be_bytes([buf[len - 2], buf[len - 1]]);

        let mut request2 = vec![ID_OPEN_CONNECTION_REQUEST_2];
        request2.extend_from_slice(&RAKNET_MAGIC);
        request2.extend_from_slice(&[4, 127, 0, 0, 1, 0x4f, 0x9f]);
        request2.extend_from_slice(&mtu.to_be_bytes());
        request2.extend_from_slice(&7i64.to_be_bytes());
        client.send_to(&request2, addr).unwrap();
        server.process_packets();
        let (len2, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(buf[0], ID_OPEN_CONNECTION_REPLY_2, "len={len2}");

        let mut incoming = vec![ID_NEW_INCOMING_CONNECTION];
        write_address(&mut incoming, client.local_addr().unwrap());
        incoming.extend_from_slice(&[0u8; 8]);
        client.send_to(&incoming, addr).unwrap();
        server.process_packets();
    }

    fn recv_game_payload(client: &UdpSocket) -> Vec<u8> {
        let mut buf = [0u8; 4096];
        for _ in 0..8 {
            let (len, _) = client.recv_from(&mut buf).unwrap();
            if buf[0] != ID_DATAGRAM {
                continue; // ACK/NACK
            }
            let frames = decode_frames(&buf[4..len]);
            assert!(!frames.is_empty());
            return frames.into_iter().map(|frame| frame.payload).flatten().collect();
        }
        panic!("no game datagram received");
    }

    fn send_frames(client: &UdpSocket, server: &BedrockNetworkServer, payload: &[u8], sequence: u32) {
        let frames = split_payload(payload, 1400, 0, 0);
        let datagram = encode_datagram(sequence, &frames);
        client.send_to(&datagram, server.local_addr().unwrap()).unwrap();
        server.process_packets();
    }

    #[test]
    fn network_settings_and_login_rejection_roundtrip() {
        let logger = Arc::new(Logger::new("test"));
        let server = BedrockNetworkServer::bind("127.0.0.1:0", "Test", 10, logger).unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(std::time::Duration::from_millis(500))).unwrap();
        raknet_handshake(&client, &server);

        // 1. RequestNetworkSettings -> uncompressed NetworkSettings batch.
        send_frames(&client, &server, &encode_batch(&[vec![REQUEST_NETWORK_SETTINGS]]), 1);
        let settings_payload = recv_game_payload(&client);
        let packets = decode_batch(&settings_payload).expect("settings batch");
        assert_eq!(packets[0][0], NETWORK_SETTINGS);
        assert_eq!(u16::from_le_bytes([packets[0][1], packets[0][2]]), 256);

        // 2. Compressed Login batch -> compressed PlayStatus rejection.
        send_frames(&client, &server, &compress_batch(&encode_batch(&[build_login_packet()]), 256), 2);
        let status_payload = recv_game_payload(&client);
        let batch = decompress_batch(&status_payload).expect("status compression");
        let packets = decode_batch(&batch).expect("status batch");
        assert_eq!(packets[0][0], PLAY_STATUS);
        assert_eq!(
            i32::from_be_bytes([packets[0][1], packets[0][2], packets[0][3], packets[0][4]]),
            PLAY_STATUS_LOGIN_FAILED_SERVER_OLD
        );
        assert_eq!(server.stats().logins, 1);
    }

    #[test]
    fn parses_modern_auth_info_with_client_key() {
        let group = EcGroup::from_curve_name(Nid::SECP384R1).unwrap();
        let client = PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap();
        let client_der = client.public_key_to_der().unwrap();

        let packet = build_auth_login(&client_der);
        let identity = parse_login(&packet).expect("modern login must parse");
        assert_eq!(identity.display_name, "Steve");
        assert_eq!(identity.identity, "00000000-0000-0000-0000-000000000001");
        assert_eq!(identity.protocol, pm_core::CURRENT_PROTOCOL as u32);
        assert_eq!(identity.client_public_key.as_deref(), Some(client_der.as_slice()));
    }

    #[test]
    fn encrypted_handshake_login_success_roundtrip() {
        let logger = Arc::new(Logger::new("test"));
        let server = BedrockNetworkServer::bind("127.0.0.1:0", "Test", 10, logger).unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(std::time::Duration::from_millis(800))).unwrap();
        raknet_handshake(&client, &server);

        // NetworkSettings (uncompressed), then Login (compressed, not yet encrypted).
        send_frames(&client, &server, &encode_batch(&[vec![REQUEST_NETWORK_SETTINGS]]), 1);
        let settings = decode_batch(&recv_game_payload(&client)).unwrap();
        assert_eq!(settings[0][0], NETWORK_SETTINGS);

        let group = EcGroup::from_curve_name(Nid::SECP384R1).unwrap();
        let client_key = PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap();
        let client_der = client_key.public_key_to_der().unwrap();
        let login = build_auth_login(&client_der);
        send_frames(&client, &server, &compress_batch(&encode_batch(&[login]), 256), 2);

        // ServerToClientHandshake (compressed, unencrypted).
        let handshake = decode_batch(&decompress_batch(&recv_game_payload(&client)).unwrap()).unwrap();
        assert_eq!(handshake[0][0], SERVER_TO_CLIENT_HANDSHAKE);
        let mut offset = 1usize;
        let jwt_len = read_varint(&handshake[0], &mut offset).unwrap() as usize;
        let jwt = std::str::from_utf8(&handshake[0][offset..offset + jwt_len]).unwrap();
        let (server_der, salt) = encryption::parse_handshake_jwt(jwt).expect("handshake JWT");
        assert!(encryption::verify_handshake_jwt(jwt, &server_der));

        // Client derives the same AES key and answers with an encrypted
        // ClientToServerHandshake.
        let server_pub = encryption::public_key_from_der(&server_der).unwrap();
        let shared = {
            let mut deriver = openssl::derive::Deriver::new(&client_key).unwrap();
            deriver.set_peer(&server_pub).unwrap();
            deriver.derive_to_vec().unwrap()
        };
        let key = derive_aes_key(&shared, &salt);
        let mut client_crypto = EncryptionContext::new(key).unwrap();

        let hello = compress_batch(&encode_batch(&[vec![CLIENT_TO_SERVER_HANDSHAKE]]), 256);
        let encrypted = client_crypto.encrypt(&hello);
        send_frames(&client, &server, &encrypted, 3);
        assert_eq!(server.stats().encryptions, 1);

        // PlayStatus(LoginSuccess) and Disconnect arrive as two encrypted batches.
        let mut packets = Vec::new();
        for _ in 0..2 {
            let encrypted = recv_game_payload(&client);
            let decrypted = client_crypto.decrypt(&encrypted).expect("server packet decrypt");
            let batch = decompress_batch(&decrypted).expect("server packet decompress");
            packets.extend(decode_batch(&batch).unwrap());
        }
        assert_eq!(packets[0][0], PLAY_STATUS);
        assert_eq!(
            i32::from_be_bytes([packets[0][1], packets[0][2], packets[0][3], packets[0][4]]),
            PLAY_STATUS_LOGIN_SUCCESS
        );
        assert_eq!(packets[1][0], DISCONNECT);
        assert_eq!(server.stats().logins, 1);
    }

    #[test]
    fn answers_unconnected_ping() {
        let logger = Arc::new(Logger::new("test"));
        let server = BedrockNetworkServer::bind("127.0.0.1:0", "Test MOTD", 10, logger).unwrap();
        let bind = server.local_addr().unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(std::time::Duration::from_millis(500))).unwrap();
        let mut ping = vec![ID_UNCONNECTED_PING];
        ping.extend_from_slice(&123i64.to_be_bytes());
        ping.extend_from_slice(&RAKNET_MAGIC);
        ping.extend_from_slice(&99i64.to_be_bytes());
        client.send_to(&ping, bind).unwrap();

        server.process_packets();
        let mut buf = [0u8; 2048];
        let (len, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(buf[0], ID_UNCONNECTED_PONG);
        assert_eq!(i64::from_be_bytes(buf[1..9].try_into().unwrap()), 123);
        let text_len = u16::from_be_bytes([buf[33], buf[34]]) as usize;
        let text = String::from_utf8_lossy(&buf[35..35 + text_len]);
        assert!(text.starts_with("MCPE;Test MOTD;"));
        assert!(text.contains(";2225;"));
        assert_eq!(len, 35 + text_len);
    }
}
