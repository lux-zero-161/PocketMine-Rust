use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub const RAKNET_MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe,
    0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

pub const ID_UNCONNECTED_PING: u8 = 0x01;
pub const ID_UNCONNECTED_PING_OPEN: u8 = 0x02;
pub const ID_OPEN_CONNECTION_REQUEST_1: u8 = 0x05;
pub const ID_OPEN_CONNECTION_REPLY_1: u8 = 0x06;
pub const ID_OPEN_CONNECTION_REQUEST_2: u8 = 0x07;
pub const ID_OPEN_CONNECTION_REPLY_2: u8 = 0x08;
pub const ID_UNCONNECTED_PONG: u8 = 0x1c;

pub struct BedrockNetworkServer {
    socket: UdpSocket,
    server_guid: u64,
    motd: String,
    online_players: usize,
    max_players: usize,
    running: Arc<AtomicBool>,
    logger: Arc<pm_core::Logger>,
}

impl BedrockNetworkServer {
    pub fn bind(
        address: &str,
        motd: impl Into<String>,
        max_players: usize,
        logger: Arc<pm_core::Logger>,
    ) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(address)?;
        socket.set_nonblocking(true)?;

        let server_guid = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        Ok(Self {
            socket,
            server_guid,
            motd: motd.into(),
            online_players: 0,
            max_players,
            running: Arc::new(AtomicBool::new(true)),
            logger,
        })
    }

    pub fn set_online_count(&mut self, count: usize) {
        self.online_players = count;
    }

    pub fn build_motd_payload(&self) -> String {
        format!(
            "MCPE;{};{};{};{};{};{};{};Survival;1;19132;19133;",
            self.motd,
            pm_core::CURRENT_PROTOCOL,
            pm_core::MINECRAFT_VERSION,
            self.online_players,
            self.max_players,
            self.server_guid,
            "PocketMine-Rust World"
        )
    }

    pub fn process_packets(&mut self) {
        let mut buf = [0u8; 2048];
        while let Ok((amt, src)) = self.socket.recv_from(&mut buf) {
            if amt == 0 {
                continue;
            }

            let packet_id = buf[0];
            match packet_id {
                ID_UNCONNECTED_PING | ID_UNCONNECTED_PING_OPEN => {
                    self.handle_unconnected_ping(&buf[..amt], src);
                }
                ID_OPEN_CONNECTION_REQUEST_1 => {
                    self.handle_open_conn_request_1(&buf[..amt], src);
                }
                ID_OPEN_CONNECTION_REQUEST_2 => {
                    self.handle_open_conn_request_2(&buf[..amt], src);
                }
                _ => {
                    // Frame sets, RakNet session packets
                }
            }
        }
    }

    fn handle_unconnected_ping(&self, data: &[u8], src: SocketAddr) {
        if data.len() < 32 {
            return;
        }

        let timestamp = &data[1..9];
        let motd_string = self.build_motd_payload();
        let motd_bytes = motd_string.as_bytes();
        let motd_len = motd_bytes.len() as u16;

        let mut response = Vec::with_capacity(35 + motd_bytes.len());
        response.push(ID_UNCONNECTED_PONG);
        response.extend_from_slice(timestamp);
        response.extend_from_slice(&self.server_guid.to_be_bytes());
        response.extend_from_slice(&RAKNET_MAGIC);
        response.extend_from_slice(&motd_len.to_be_bytes());
        response.extend_from_slice(motd_bytes);

        let _ = self.socket.send_to(&response, src);
    }

    fn handle_open_conn_request_1(&self, data: &[u8], src: SocketAddr) {
        if data.len() < 18 {
            return;
        }

        let mtu_size = (data.len() + 28) as u16; // UDP header (8) + IP header (20)
        let mut response = Vec::with_capacity(28);
        response.push(ID_OPEN_CONNECTION_REPLY_1);
        response.extend_from_slice(&RAKNET_MAGIC);
        response.extend_from_slice(&self.server_guid.to_be_bytes());
        response.push(0x00); // Has security = false
        response.extend_from_slice(&mtu_size.to_be_bytes());

        let _ = self.socket.send_to(&response, src);
        self.logger.debug(&format!("Sent OpenConnectionReply 1 to {} (MTU: {})", src, mtu_size));
    }

    fn handle_open_conn_request_2(&self, data: &[u8], src: SocketAddr) {
        if data.len() < 34 {
            return;
        }

        let mtu_size = u16::from_be_bytes([data[data.len() - 2], data[data.len() - 1]]);
        let mut response = Vec::with_capacity(35);
        response.push(ID_OPEN_CONNECTION_REPLY_2);
        response.extend_from_slice(&RAKNET_MAGIC);
        response.extend_from_slice(&self.server_guid.to_be_bytes());
        
        // Client address
        match src {
            SocketAddr::V4(v4) => {
                response.push(4); // IPv4
                response.extend_from_slice(&v4.ip().octets());
                response.extend_from_slice(&v4.port().to_be_bytes());
            }
            SocketAddr::V6(v6) => {
                response.push(6); // IPv6
                response.extend_from_slice(&v6.ip().octets());
                response.extend_from_slice(&v6.port().to_be_bytes());
            }
        }
        response.extend_from_slice(&mtu_size.to_be_bytes());
        response.push(0x00); // Encryption disabled

        let _ = self.socket.send_to(&response, src);
        self.logger.debug(&format!("Sent OpenConnectionReply 2 to {}", src));
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}
