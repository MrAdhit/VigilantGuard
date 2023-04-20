use std::{time::Duration, thread, borrow::Cow};

use log::info;
use serde_json::Value;
use tokio::net::tcp::OwnedReadHalf;
use valence_protocol::{text::Text, bytes::BytesMut, packet::s2c::login::LoginDisconnectS2c};

use crate::{RUNTIME, IP_CACHE, packet::{QueryResponseS2cOwn, HandshakeC2sOwn}, make_bytes, CONNECTIONS, guardian::ip_blacklisted, file::{VIGILANT_CONFIG, VIGILANT_LANG}};

pub fn ip_cache(reader: &OwnedReadHalf) {
    let ip = reader.peer_addr().unwrap().ip().to_string();
    if VIGILANT_CONFIG.guardian.ping_protection {
        RUNTIME.spawn(async move {
            let timestamp = chrono::Utc::now().timestamp();

            if let Some(_) = { IP_CACHE.lock().await.values().find(|&v| v == &ip) } {
                return;
            }

            { IP_CACHE.lock().await.insert(timestamp, ip) };

            thread::sleep(Duration::from_secs(10));

            IP_CACHE.lock().await.remove(&timestamp);
        });
        info!("Caching IP");
    }
}

pub fn ip_forward(packet: &mut HandshakeC2sOwn, reader: &OwnedReadHalf) {
    if VIGILANT_CONFIG.proxy.ip_forward {
        packet.server_address = format!("{addr}|{player_addr}", addr = packet.server_address, player_addr = reader.peer_addr().unwrap().ip().to_string());
    }
}

pub fn query_response(packet: &mut QueryResponseS2cOwn) {
    if !VIGILANT_CONFIG.proxy.motd_forward {
        let json = serde_json::from_str::<Value>(&packet.json).unwrap();
        let source = packet.json.to_string();
    
        let description = Text::from("Intercepted with VigilantGuard");
        let description_from = serde_json::to_string(&json["description"]).unwrap();
        let description_to = serde_json::to_string(&description).unwrap();
    
        let version_name = "VigilantGuard";
        let version_from = serde_json::to_string(&json["version"]["name"]).unwrap();
        let version_to = serde_json::to_string(&version_name).unwrap();
    
        let source = source.replace(&description_from, &description_to).replace(&version_from, &version_to);
    
        packet.json = source;
    }
}

pub async fn vpn_filter(reader: &OwnedReadHalf) -> Option<BytesMut> {
    let ip = reader.peer_addr().unwrap().ip().to_string();

    if VIGILANT_CONFIG.guardian.vpn_filter {
        if ip_blacklisted(ip).await {
            return Some(make_bytes!(LoginDisconnectS2c { reason: Cow::Owned(Text::from(VIGILANT_LANG.player_ip_blacklisted_kick.clone())) }))
        }
    }

    None
}

pub fn concurrency_filter(reader: &OwnedReadHalf) -> Option<BytesMut> {
    let ip = reader.peer_addr().unwrap().ip().to_string();

    if let Ok(lock) = CONNECTIONS.try_lock() {
        if lock.get(&ip).unwrap() > &VIGILANT_CONFIG.guardian.ip_concurrent_limit {
            return Some(make_bytes!(LoginDisconnectS2c { reason: Cow::Owned(Text::from(VIGILANT_LANG.player_connection_more_kick.clone())) }))
        }
    }

    None
}

pub async fn ping_filter(reader: &OwnedReadHalf) -> Option<BytesMut> {
    let ip = reader.peer_addr().unwrap().ip().to_string();

    if VIGILANT_CONFIG.guardian.ping_protection {
        if let None = IP_CACHE.lock().await.values().find(|&v| v == &ip) {
            return Some(make_bytes!(LoginDisconnectS2c { reason: Cow::Owned(Text::from(VIGILANT_LANG.player_ping_not_cached_kick.clone())) }))
        }
    }

    None
}
