use std::{collections::HashMap, sync::Arc};

use serde_json::Value;
use tokio::{net::tcp::{OwnedReadHalf, OwnedWriteHalf}, sync::Mutex};
use valence_protocol::{Encode, Decode, text::Text, var_int::VarInt, packet::s2c::login::LoginQueryRequestS2c};

use crate::{packet::*, *};


pub struct FrontInterceptor<'a> {
    pub reader: &'a mut OwnedReadHalf,
    pub writer: &'a mut OwnedWriteHalf,
    pub raw_buffer: [u8; 4096],
    pub intercepted: bool,
}

impl<'a> FrontInterceptor<'a> {
    pub fn new(reader: &'a mut OwnedReadHalf, writer: &'a mut OwnedWriteHalf) -> Self {
        Self {
            reader,
            writer,
            raw_buffer: [0u8; 4096],
            intercepted: false,
        }
    }

    async fn peek(&mut self) -> usize {
        self.reader.peek(&mut self.raw_buffer).await.unwrap()
    }

    pub async fn reply_ping(&mut self) {
        let peek_len = self.peek().await;

        self.writer.write(&self.raw_buffer[..peek_len]).await.unwrap();

        self.intercepted();
    }

    fn intercepted(&mut self) {
        self.intercepted = true;
    }
}

pub struct MiddleInterceptor<'a> {
    pub reader: &'a mut OwnedReadHalf,
    pub writer: &'a mut OwnedWriteHalf,
    pub raw_buffer: &'a [u8; 4096],
    pub vector_buffer: Vec<u8>,
    pub array_buffer: &'a [u8],
    pub response_buffer: Vec<u8>,
    pub passthrough: bool,
    pub connections: Arc<Mutex<HashMap<String, usize>>>
}

impl<'a> MiddleInterceptor<'a> {
    pub fn new(reader: &'a mut OwnedReadHalf, writer: &'a mut OwnedWriteHalf, raw_buffer: &'a [u8; 4096], buffer_length: usize, connections: Arc<Mutex<HashMap<String, usize>>>) -> MiddleInterceptor<'a> {
        let vector_buffer = raw_buffer[..buffer_length].iter().cloned().collect();
        let array_buffer = &raw_buffer[..];

        Self {
            reader,
            writer,
            raw_buffer,
            vector_buffer,
            array_buffer,
            response_buffer: Vec::new(),
            passthrough: false,
            connections
        }
    }

    pub fn passthrough(&mut self) {
        self.passthrough = true;
    }

    fn modify_buffer(&mut self, buffer: Vec<u8>) {
        self.response_buffer = buffer;
    }

    pub async fn c2s_handshake(&mut self, ping_ip_cache: Arc<Mutex<HashMap<i64, String>>>, packet_state: Arc<Mutex<PacketState>>) {
        if let Ok(packet) = C2sHandshakePacket::decode(&mut self.array_buffer) {
            *packet_state.lock().await = packet.next;

            encode_packet!(0x00, packet, self.response_buffer);

            match packet.next {
                PacketState::Status => {
                    if let Ok(packet) = C2sQueryRequest::decode(&mut self.array_buffer) {
                        encode_packet!(packet, self.response_buffer);
                    }

                    if PING_PROTECTION {
                        let player_ip = self.reader.peer_addr().unwrap().ip().to_string();
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        
                        rt.spawn(async move {
                            let timestamp = chrono::Utc::now().timestamp();

                            {
                                let mut ip_cache = ping_ip_cache.lock().await;

                                if let Some(_) = ip_cache.values().find(|&v| v == &player_ip) {
                                    return;
                                }

                                ip_cache.insert(timestamp, player_ip);
                            }

                            println!("New ip cached");

                            std::thread::sleep(std::time::Duration::from_secs(300));

                            ping_ip_cache.lock().await.remove(&timestamp);
                            println!("Deleted an ip");
                        });
                        rt.shutdown_background();
                    }
                },
                _ => {
                    self.passthrough();
                }
            }

        }
    }

    pub async fn s2c_status(&mut self) {
        if let Ok(mut packet) = S2cQueryResponse::decode(&mut self.array_buffer) {
            let json = serde_json::from_str::<Value>(packet.json).unwrap();
            let source = packet.json.to_string();

            let description = Text::from("Intercepted with VigilantGuard");
            let description_from = serde_json::to_string(&json["description"]).unwrap();
            let description_to = serde_json::to_string(&description).unwrap();

            let version_name = "VigilantGuard";
            let version_from = serde_json::to_string(&json["version"]["name"]).unwrap();
            let version_to = serde_json::to_string(&version_name).unwrap();

            let source = source.replace(&description_from, &description_to).replace(&version_from, &version_to);

            packet.json = &source;
            
            encode_packet!(0x00, packet, self.response_buffer);
        }
    }

    pub async fn s2c_login(&mut self, ping_ip_cache: Arc<Mutex<HashMap<i64, String>>>) {
        let player_ip = self.writer.peer_addr().unwrap().ip().to_string();

        let connections = self.connections.lock().await.clone();
        let connections = connections.get(&player_ip).unwrap();

        if connections > &IP_CONCURRENT_LIMIT {
            dbg!(connections);
            self.disconnect_with_reason(Text::from("More connection"));
            dbg!("conn rejected");
            return;
        }

        if PING_PROTECTION {
            if let None = ping_ip_cache.lock().await.values().find(|&v| v == &player_ip) {
                self.disconnect_with_reason(Text::from("Not cached"));
                return;
            }
        }

        self.s2c_login_plugin().await;

        self.passthrough();
    }

    async fn s2c_login_plugin(&mut self) {
        let packet_header = PacketHeader::decode(&mut self.array_buffer).expect("Failed to decode packet header");
        dbg!(packet_header);
        if let Ok(packet) = LoginQueryRequestS2c::decode(&mut self.array_buffer) {
            dbg!(packet.channel);
        }
    }

    fn disconnect_with_reason(&mut self, reason: Text) {
        let mut writer = Vec::new();
        let packet = S2cDisconnect { reason: std::borrow::Cow::Owned(Text::from(reason)) };

        encode_packet!(0x00, packet, writer);

        self.modify_buffer(writer);
    }
}