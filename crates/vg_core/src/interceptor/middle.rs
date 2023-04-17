use std::{sync::Arc, collections::HashMap, time::Duration};

use log::info;
use serde_json::Value;
use tokio::{net::tcp::{OwnedReadHalf, OwnedWriteHalf}, sync::Mutex, io::{AsyncWriteExt, AsyncReadExt}};
use valence_protocol::{Decode, text::Text};

use crate::{packet::*, *};

pub struct Interceptor<'a> {
    reader: &'a mut OwnedReadHalf,
    writer: &'a mut OwnedWriteHalf,
    raw_buffer: &'a [u8; 4096],
    vector_buffer: Vec<u8>,
    array_buffer: &'a [u8],
    response_buffer: Vec<u8>,
    passthrough: bool,
    connections: Arc<Mutex<HashMap<String, usize>>>
}

impl<'a> Interceptor<'a> {
    pub async fn init(reader: &'a mut OwnedReadHalf, writer: &'a mut OwnedWriteHalf, raw_buffer: &'a mut [u8; 4096], connections_arc: Arc<Mutex<HashMap<String, usize>>>, packet_stage_arc: Arc<Mutex<PacketStage>>, ip_cache: Arc<Mutex<HashMap<i64, String>>>) -> (usize, Option<Interceptor<'a>>) {        
        let read_len = reader.read(raw_buffer).await.unwrap_or(0);
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

        let _write_len = interceptor.writer.write(response).await.unwrap_or(0);

        (read_len, Some(interceptor))
    }

    pub fn passthrough(&mut self) {
        self.passthrough = true;
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
                    
                    if PING_PROTECTION {
                        self.info_log("Caching IP");
                        RUNTIME.spawn(async move {
                            let timestamp = chrono::Utc::now().timestamp();
    
                            {
                                let mut ip_cache = ip_cache.lock().await;
                                if let Some(_) = ip_cache.values().find(|&v| v == &ip) {
                                    return;
                                }
                                ip_cache.insert(timestamp, ip);
                            }
    
                            std::thread::sleep(Duration::from_secs(10));
    
                            ip_cache.lock().await.remove(&timestamp);
                        });
                    }

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

    fn info_log(&self, msg: &str) {
        info!("{}", colorizer!("[/c(dark_blue){}c(reset)] {}", self.reader.peer_addr().unwrap().to_string(), msg));
    }
}