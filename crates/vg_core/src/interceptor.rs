use std::{collections::HashMap, sync::Arc};

use serde_json::Value;
use tokio::{net::tcp::{OwnedReadHalf, OwnedWriteHalf}, sync::Mutex};
use valence_protocol::{Encode, Decode, text::Text, var_int::VarInt, packet::{s2c::login::LoginQueryRequestS2c, c2s::login::LoginHelloC2s}};

use crate::{packet::*, *};


pub struct FrontInterceptor<'a> {
    pub reader: &'a mut OwnedReadHalf,
    pub writer: &'a mut OwnedWriteHalf,
    pub raw_buffer: [u8; 4096],
    pub intercepted: bool,
}

impl<'a> FrontInterceptor<'a> {
    pub async fn init(reader: &'a mut OwnedReadHalf, writer: &'a mut OwnedWriteHalf, packet_stage: Arc<Mutex<PacketStage>>) -> FrontInterceptor<'a> {
        let mut interceptor = FrontInterceptor {
            reader,
            writer,
            raw_buffer: [0u8; 4096],
            intercepted: false,
        };

        match packet_stage.lock().await.clone() {
            PacketStage::C2sPingRequest => {
                interceptor.reply_ping().await;
            }
            _ => { }
        }

        interceptor
    }

    async fn reply_ping(&mut self) {
        let peek_len = self.peek().await;

        self.writer.write(&self.raw_buffer[..peek_len]).await.unwrap();

        self.intercepted();
    }

    async fn peek(&mut self) -> usize {
        self.reader.peek(&mut self.raw_buffer).await.unwrap()
    }

    fn intercepted(&mut self) {
        self.intercepted = true;
    }
}

pub struct MiddleInterceptor<'a> {
    reader: &'a mut OwnedReadHalf,
    writer: &'a mut OwnedWriteHalf,
    raw_buffer: &'a [u8; 4096],
    vector_buffer: Vec<u8>,
    array_buffer: &'a [u8],
    response_buffer: Vec<u8>,
    passthrough: bool,
    connections: Arc<Mutex<HashMap<String, usize>>>
}

impl<'a> MiddleInterceptor<'a> {
    pub async fn init(reader: &'a mut OwnedReadHalf, writer: &'a mut OwnedWriteHalf, raw_buffer: &'a mut [u8; 4096], connections_arc: Arc<Mutex<HashMap<String, usize>>>, packet_stage_arc: Arc<Mutex<PacketStage>>, ip_cache: Arc<Mutex<HashMap<i64, String>>>) -> (usize, Option<MiddleInterceptor<'a>>) {        let read_len = reader.read(raw_buffer).await.unwrap_or(0);
        if read_len == 0 { return (0, None) }

        let vector_buffer = raw_buffer[..read_len].iter().cloned().collect();
        let array_buffer = &raw_buffer[..];

        let mut interceptor = Self {
            reader,
            writer,
            raw_buffer,
            vector_buffer,
            array_buffer,
            response_buffer: Vec::new(),
            passthrough: false,
            connections: connections_arc
        };

        loop {
            let packet_stage = packet_stage_arc.lock().await.clone();
            match packet_stage {
                PacketStage::C2sHandshake => {
                    interceptor.c2s_handshake(ip_cache.clone(), packet_stage_arc.clone()).await;
                },
                PacketStage::C2sQueryRequest => {
                    if let Ok(mut packet) = C2sQueryRequest::decode(&mut interceptor.array_buffer) {
                        interceptor.response_buffer.append(&mut packet.to_buffer())
                    }
        
                    *packet_stage_arc.lock().await = PacketStage::S2cQueryResponse;
                },
                PacketStage::S2cQueryResponse => {
                    interceptor.s2c_status().await;
        
                    *packet_stage_arc.lock().await = PacketStage::C2sPingRequest;
                },
                PacketStage::C2sPingRequest => {
                    if let Ok(mut packet) = C2sPingRequest::decode(&mut interceptor.array_buffer) {
                        interceptor.response_buffer.append(&mut packet.to_buffer());
                    }
        
                    *packet_stage_arc.lock().await = PacketStage::S2cPingResponse;
                    continue;
                },
                PacketStage::C2sLoginStart => {
                    if let Ok(mut packet) = C2sLoginStart::decode(&mut interceptor.array_buffer) {
                        interceptor.response_buffer.append(&mut packet.to_buffer());
                        dbg!(packet);
                    }
        
                    *packet_stage_arc.lock().await = PacketStage::S2cEncryptionRequest;
                },
                PacketStage::S2cPingResponse => { interceptor.passthrough(); },
                PacketStage::S2cEncryptionRequest => { interceptor.passthrough(); },
            }

            if format!("{:?}", packet_stage).contains("C2s") != format!("{:?}", packet_stage_arc.lock().await).contains("C2s") {
                break;
            }

            if interceptor.passthrough {
                break;
            }
        }

        let response = if interceptor.passthrough { &interceptor.vector_buffer } else { &interceptor.response_buffer };

        let write_len = interceptor.writer.write(response).await.unwrap_or(0);

        (read_len, Some(interceptor))
    }

    pub fn passthrough(&mut self) {
        self.passthrough = true;
    }

    fn modify_buffer(&mut self, buffer: Vec<u8>) {
        self.response_buffer = buffer;
    }

    async fn c2s_handshake(&mut self, ip_cache: Arc<Mutex<HashMap<i64, String>>>, packet_stage_arc: Arc<Mutex<PacketStage>>) {
        if let Ok(mut packet) = C2sHandshakePacket::decode(&mut self.array_buffer) {
            if IP_FORWARD {
                packet.addr = format!("{addr}|{player_addr}", addr = packet.addr, player_addr = self.reader.peer_addr().unwrap().ip().to_string());
            }

            self.response_buffer.append(&mut packet.to_buffer());

            match packet.next {
                PacketState::Status => {
                    let ip = self.reader.peer_addr().unwrap().ip().to_string();
                    let rt = Runtime::new().unwrap();

                    rt.spawn(async move {
                        let timestamp = chrono::Utc::now().timestamp();

                        {
                            let mut ip_cache = ip_cache.lock().await;

                            if let Some(_) = ip_cache.values().find(|&v| v == &ip) {
                                return;
                            }

                            ip_cache.insert(timestamp, ip);
                        }

                        tokio::time::sleep(Duration::from_secs(10)).await;

                        ip_cache.lock().await.remove(&timestamp);
                    });
                    rt.shutdown_background();

                    *packet_stage_arc.lock().await = PacketStage::C2sQueryRequest;
                }
                PacketState::Login => {
                    *packet_stage_arc.lock().await = PacketStage::C2sLoginStart;
                }
                _ => { }
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
            
            self.response_buffer.append(&mut packet.to_buffer());
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

        self.passthrough();
    }

    fn disconnect_with_reason(&mut self, reason: Text) {
        let mut writer = Vec::new();
        let packet = S2cDisconnect { reason: std::borrow::Cow::Owned(Text::from(reason)) };

        encode_packet!(0x00, packet, writer);

        self.modify_buffer(writer);
    }
}