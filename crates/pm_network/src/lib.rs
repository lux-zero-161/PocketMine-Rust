//! RakNet / Bedrock network layer.
//!
//! Implemented milestones:
//! * Unconnected ping/pong, so the server appears in the Minecraft LAN list.
//! * RakNet offline handshake (request 1/2, connection request, new incoming
//!   connection) with MTU negotiation.
//! * Datagram + encapsulated frame codec (reliable ordered) used by Bedrock.
//! * `RequestNetworkSettings` -> `NetworkSettings` (compression negotiation).
//!
//! The encrypted JWT login chain and the full packet handler are the next
//! milestone; `Login` is currently answered with a PlayStatus so clients fail
//! cleanly instead of hanging.

use pm_core::{Logger, RAKNET_MAGIC};
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
    pub const REQUEST_NETWORK_SETTINGS: u8 = 0x5a;
    pub const NETWORK_SETTINGS: u8 = 0x8f;
    pub const REQUEST_CHUNK_RADIUS: u8 = 0x45;
    pub const CHUNK_RADIUS_UPDATED: u8 = 0x46;
    pub const SET_LOCAL_PLAYER_AS_INITIALIZED: u8 = 0x71;
    pub const REQUEST_CAPABILITIES: u8 = 0x77;

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

// ---------------------------------------------------------------------------
// Helpers
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
            buf.extend_from_slice(&[0u8; 8]); // address family
            buf.extend_from_slice(&[0u8; 2]); // port
            buf.extend_from_slice(&[0u8; 4]); // flow info
            buf.extend_from_slice(&v6.ip().octets());
            buf.extend_from_slice(&v6.port().to_be_bytes());
        }
    }
}

fn read_magic(buf: &[u8], offset: usize) -> bool {
    buf.len() >= offset + 16 && &buf[offset..offset + 16] == RAKNET_MAGIC
}

// ---------------------------------------------------------------------------
// Encapsulated frame codec
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub reliability: u8,
    pub reliable_index: Option<u32>,
    pub sequence_index: Option<u32>,
    pub order_index: Option<u32>,
    pub order_channel: Option<u8>,
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
            payload,
        }
    }

    pub fn encode(&self, out: &mut Vec<u8>) {
        out.push(0x80 | (self.reliability << 5)); // "is frame set" bit + reliability
        let len = self.payload.len() as u16;
        out.extend_from_slice(&len.to_be_bytes());
        let mut reliable = false;
        let mut sequenced = false;
        let mut ordered = false;
        match self.reliability {
            2 | 3 | 4 | 6 | 7 => reliable = true,
            _ => {}
        }
        match self.reliability {
            1 | 4 => sequenced = true,
            3 | 7 => ordered = true,
            _ => {}
        }
        if reliable {
            if let Some(i) = self.reliable_index {
                out.extend_from_slice(&i.to_le_bytes());
            }
        }
        if sequenced {
            if let Some(i) = self.sequence_index {
                out.extend_from_slice(&i.to_le_bytes());
            }
        }
        if ordered {
            if let Some(o) = self.order_index {
                out.extend_from_slice(&o.to_le_bytes());
                out.push(self.order_channel.unwrap_or(0));
            }
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
        let length = u16::from_be_bytes([buf[i], buf[i + 1]]) as usize;
        i += 2;
        let mut reliable_index = None;
        let mut sequence_index = None;
        let mut order_index = None;
        let mut order_channel = None;
        let mut reliable = false;
        let mut sequenced = false;
        let mut ordered = false;
        match reliability {
            2 | 3 | 4 | 6 | 7 => reliable = true,
            _ => {}
        }
        match reliability {
            1 | 4 => sequenced = true,
            3 | 7 => ordered = true,
            _ => {}
        }
        if reliable && i + 4 <= buf.len() {
            reliable_index = Some(u32::from_le_bytes([
                buf[i],
                buf[i + 1],
                buf[i + 2],
                buf[i + 3],
            ]));
            i += 4;
        }
        if sequenced && i + 4 <= buf.len() {
            sequence_index = Some(u32::from_le_bytes([
                buf[i],
                buf[i + 1],
                buf[i + 2],
                buf[i + 3],
            ]));
            i += 4;
        }
        if ordered && i + 5 <= buf.len() {
            order_index = Some(u32::from_le_bytes([
                buf[i],
                buf[i + 1],
                buf[i + 2],
                buf[i + 3],
            ]));
            order_channel = Some(buf[i + 4]);
            i += 5;
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
            payload: buf[i..i + length].to_vec(),
        });
        i += length;
    }
    frames
}

fn encode_datagram(sequence: u32, frames: &[Frame]) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
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

#[derive(Debug, Clone)]
pub struct Session {
    pub address: SocketAddr,
    pub mtu: u16,
    pub state: SessionState,
    pub datagram_sequence: u32,
    pub reliable_index: u32,
    pub order_index: u32,
    pub last_seen: i64,
    pub compression_threshold: u16,
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
        }
    }

    fn next_datagram_sequence(&mut self) -> u32 {
        let seq = self.datagram_sequence;
        self.datagram_sequence = self.datagram_sequence.wrapping_add(1) & 0x00ff_ffff;
        seq
    }

    fn next_reliable(&mut self) -> u32 {
        let idx = self.reliable_index;
        self.reliable_index = self.reliable_index.wrapping_add(1);
        idx
    }

    fn next_order(&mut self) -> u32 {
        let idx = self.order_index;
        self.order_index = self.order_index.wrapping_add(1);
        idx
    }
}

#[derive(Debug, Default, Clone)]
pub struct NetworkStats {
    pub pings: u64,
    pub pongs: u64,
    pub handshakes: u64,
    pub datagrams_in: u64,
    pub datagrams_out: u64,
    pub frames_in: u64,
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
    /// Callback invoked for each decoded Bedrock game packet.
    packet_handler: Arc<RwLock<Option<Box<dyn FnMut(SocketAddr, &[u8]) + Send + Sync>>>>,
    next_guid: AtomicU64,
    compression_threshold: u16,
}

impl BedrockNetworkServer {
    pub fn bind(
        bind_addr: &str,
        motd: &str,
        max_players: usize,
        logger: Arc<Logger>,
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        let local = socket.local_addr()?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let guid = (now.as_nanos() as i64).wrapping_mul(31) | 1;

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
            port_v6: local.port() + 1,
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
            compression_threshold: 256,
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

        let pong_text = self.pong.read().unwrap().encode();
        let mut out = Vec::with_capacity(35 + pong_text.len());
        out.push(ID_UNCONNECTED_PONG);
        out.extend_from_slice(&ping_time.to_be_bytes());
        out.extend_from_slice(&(self.pong.read().unwrap().server_guid as u64).to_be_bytes());
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
        out.extend_from_slice(&(self.next_guid.load(Ordering::Relaxed)).to_be_bytes());
        out.push(0); // security disabled
        out.extend_from_slice(&mtu.to_be_bytes());
        let _ = self.socket.send_to(&out, addr);
        self.stats.write().unwrap().handshakes += 1;
    }

    fn handle_open_request_2(&self, addr: SocketAddr, data: &[u8]) {
        if data.len() < 19 || !read_magic(data, 1) {
            return;
        }
        // Client address follows the magic; MTU is the two bytes before the
        // 8-byte client GUID at the end.
        let mtu = if data.len() >= data.len().saturating_sub(10) + 1 {
            u16::from_be_bytes([data[data.len() - 10], data[data.len() - 9]])
        } else {
            DEFAULT_MTU
        };
        let mtu = mtu.clamp(576, 1492);
        let mut out = Vec::with_capacity(32);
        out.push(ID_OPEN_CONNECTION_REPLY_2);
        out.extend_from_slice(&RAKNET_MAGIC);
        out.extend_from_slice(&(self.next_guid.load(Ordering::Relaxed)).to_be_bytes());
        write_address(&mut out, addr);
        out.extend_from_slice(&mtu.to_be_bytes());
        out.push(0); // encryption disabled
        let _ = self.socket.send_to(&out, addr);

        self.sessions
            .write()
            .unwrap()
            .insert(addr, Session::new(addr, mtu));
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
        out.extend_from_slice(&0u16.to_be_bytes()); // system index
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
        self.logger
            .debug(&format!("RakNet client connected: {}", addr));
    }

    fn handle_datagram_frame(&self, addr: SocketAddr, data: &[u8]) {
        if data.len() < 4 {
            return;
        }
        let sequence = u32::from_be_bytes([0, data[1], data[2], data[3]]);
        let frames = decode_frames(&data[4..]);

        self.stats.write().unwrap().datagrams_in += 1;
        self.stats.write().unwrap().frames_in += frames.len() as u64;

        for frame in frames {
            if frame.payload.is_empty() {
                continue;
            }
            let packet_id = frame.payload[0];
            match packet_id {
                REQUEST_NETWORK_SETTINGS => self.send_network_settings(addr, sequence),
                LOGIN => {
                    // Full JWT/Xbox verification and player spawn are the next
                    // milestone. Reject cleanly with a PlayStatus packet.
                    self.send_play_status(addr, PLAY_STATUS_LOGIN_FAILED_SERVER_OLD);
                    self.logger.info(&format!(
                        "Rejected login from {}: Bedrock login/encryption is not ported yet",
                        addr
                    ));
                }
                _ => {
                    if let Some(handler) = self.packet_handler.write().unwrap().as_mut() {
                        handler(addr, &frame.payload);
                    }
                }
            }
        }

        let ack = encode_ack(&[sequence]);
        let _ = self.socket.send_to(&ack, addr);
    }

    fn send_network_settings(&self, addr: SocketAddr, _sequence: u32) {
        let mut payload = Vec::with_capacity(6);
        payload.push(NETWORK_SETTINGS);
        payload.extend_from_slice(&self.compression_threshold.to_le_bytes()); // zlib threshold
        payload.extend_from_slice(&0u16.to_le_bytes()); // compression algorithm: zlib
        payload.extend_from_slice(&self.compression_threshold.to_le_bytes()); // client-side threshold

        self.send_frame_packets(addr, vec![payload]);
    }

    fn send_play_status(&self, addr: SocketAddr, status: i32) {
        let mut payload = Vec::with_capacity(5);
        payload.push(PLAY_STATUS);
        payload.extend_from_slice(&status.to_be_bytes());
        self.send_frame_packets(addr, vec![payload]);
    }

    /// Sends game packets as reliable ordered frames inside a RakNet datagram.
    pub fn send_frame_packets(&self, addr: SocketAddr, packets: Vec<Vec<u8>>) {
        let Some(mut session) = self.sessions.write().unwrap().remove(&addr) else {
            return;
        };
        let mut frames = Vec::with_capacity(packets.len());
        for payload in packets {
            let reliable = session.next_reliable();
            let order = session.next_order();
            frames.push(Frame::reliable_ordered(payload, reliable, order));
        }
        let sequence = session.next_datagram_sequence();
        self.sessions.write().unwrap().insert(addr, session);

        let datagram = encode_datagram(sequence, &frames);
        let _ = self.socket.send_to(&datagram, addr);
        self.stats.write().unwrap().datagrams_out += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn answers_unconnected_ping() {
        let logger = Arc::new(Logger::new("test"));
        let server = BedrockNetworkServer::bind("127.0.0.1:0", "Test MOTD", 10, logger).unwrap();
        let bind = server.local_addr().unwrap();

        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_millis(500)))
            .unwrap();
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
