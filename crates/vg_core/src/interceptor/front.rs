use std::{sync::Arc};

use log::info;
use tokio::{net::tcp::{OwnedReadHalf, OwnedWriteHalf}, sync::Mutex, io::{AsyncWriteExt}};
use valence_protocol::{Decode, text::Text};

use crate::{packet::*, *, guardian::ip_blacklisted};

use super::disconnect_with_reason;

pub struct Interceptor<'a> {
    pub reader: &'a mut OwnedReadHalf,
    pub writer: &'a mut OwnedWriteHalf,
    pub raw_buffer: [u8; 4096],
    pub intercepted: bool,
}

impl<'a> Interceptor<'a> {
    pub async fn init(reader: &'a mut OwnedReadHalf, writer: &'a mut OwnedWriteHalf, packet_stage: Arc<Mutex<PacketStage>>) -> Interceptor<'a> {
        let mut interceptor = Interceptor {
            reader,
            writer,
            raw_buffer: [0u8; 4096],
            intercepted: false,
        };

        match packet_stage.lock().await.clone() {
            PacketStage::C2sPingRequest => {
                interceptor.reply_ping().await;
            }
            PacketStage::C2sHandshake => {
                interceptor.filter_connection().await;
            }
            _ => { }
        }

        interceptor
    }

    async fn filter_connection(&mut self) {
        let peek_len = self.peek().await;

        if let Ok(packet) = C2sHandshakePacket::decode(&mut &self.raw_buffer[..peek_len]) {
            match packet.next {
                PacketState::Login => {
                    let ip = self.reader.peer_addr().unwrap().ip().to_string();

                    if VPN_PROTECTION {
                        if ip_blacklisted(ip.clone()).await {
                            self.writer.write(&disconnect_with_reason(Text::from("IP Blacklisted"))).await.unwrap();
                            self.info_log("Connection rejected because: IP Blacklisted");
                            self.intercepted();
                            return;
                        }
                    }

                    if CONNECTIONS.lock().await.get(&ip).unwrap() > &IP_CONCURRENT_LIMIT {
                        self.writer.write(&disconnect_with_reason(Text::from("More connection"))).await.unwrap();
                        self.info_log("Connection rejected because: Max connection excedeed");
                        self.intercepted();
                        return;
                    }

                    if PING_PROTECTION {
                        if let None = IP_CACHE.lock().await.values().find(|&v| v == &ip) {
                            self.writer.write(&disconnect_with_reason(Text::from("Not cached"))).await.unwrap();
                            self.info_log("Connection rejected because: IP is not cached");
                            self.intercepted();
                            return;
                        }
                    }
                }
                _ => { }
            }
        }
    }

    fn info_log(&self, msg: &str) {
        info!("{}", colorizer!("[/c(dark_blue){}c(reset)] {}", self.writer.peer_addr().unwrap().to_string(), msg));
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