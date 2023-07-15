use std::borrow::Cow;

use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use log::info;

use tokio::net::tcp::OwnedReadHalf;

use valence_protocol::bytes::BytesMut;
use valence_protocol::packet::s2c::login::LoginDisconnectS2c;
use valence_protocol::text::Text;

use crate::file::{VIGILANT_CONFIG, VIGILANT_LANG};
use crate::guardian::ip_blacklisted;
use crate::macros::coloriser;
use crate::packet::{c2s, s2c};
use crate::{make_bytes, CONNECTIONS, IP_CACHE, RUNTIME, SERVER_ALIVE};

use super::interceptor::InterceptResult;

macro_rules! log {
    ($msg:expr,$reader:expr) => {
        info!("{}", coloriser!("[/c(dark_blue){}c(reset)] {}", $reader.peer_addr().unwrap(), $msg));
    };
}

macro_rules! reject {
    ($reason:expr,$kick_reason:expr,$reader:expr) => {
        log!(format!("Rejected because: {}", $kick_reason), $reader);
        return Some(make_bytes!(LoginDisconnectS2c { reason: Cow::Owned(Text::from($reason)) }))
    };
}

// TODO: Finish this protection

pub struct C2S;

impl C2S {
    pub async fn handshake(packet: c2s::Handshake, _reader: &OwnedReadHalf) -> (InterceptResult, c2s::Handshake) {
        (InterceptResult::PASSTHROUGH, packet)
    }

    pub async fn query_request(packet: c2s::QueryRequest, reader: &OwnedReadHalf) -> (InterceptResult, c2s::QueryRequest) {
        if !SERVER_ALIVE.load(Ordering::Relaxed) {
            let motd = s2c::QueryResponse { json: format!("{{\n\"version\":{{\n\"name\":\"{}\",\n\"protocol\":999\n}},\n\"players\":{{\n\"max\":0,\n\"online\":0,\n\"sample\":[]\n}},\n\"description\":{{\n\"text\":\"{}\"\n}},\n\"favicon\":\"data:image/png;base64,\",\n\"enforcesSecureChat\":true\n}}", VIGILANT_LANG.server_version_name, VIGILANT_LANG.server_offline_motd) };

            return (InterceptResult::RETURN(Some(make_bytes!(motd))), packet);
        }

        ip_cache(reader);

        (InterceptResult::PASSTHROUGH, packet)
    }

    pub async fn query_ping(packet: c2s::QueryPing, _reader: &OwnedReadHalf) -> (InterceptResult, c2s::QueryPing) {
        (InterceptResult::PASSTHROUGH, packet)
    }

    pub async fn login_hello(packet: c2s::LoginHello, reader: &OwnedReadHalf) -> (InterceptResult, c2s::LoginHello) {
        if !SERVER_ALIVE.load(Ordering::Relaxed) {
            let reason = LoginDisconnectS2c { reason: Cow::Owned(Text::from(VIGILANT_LANG.server_offline_kick.clone())) };

            return (InterceptResult::RETURN(Some(make_bytes!(reason))), packet);
        }

        if let Some(bytes) = ping_filter(reader).await {
            return (InterceptResult::RETURN(Some(bytes)), packet);
        }

        (InterceptResult::PASSTHROUGH, packet)
    }
}

pub struct S2C;

impl S2C {
    pub async fn query_response(packet: s2c::QueryResponse, _reader: &OwnedReadHalf) -> (InterceptResult, s2c::QueryResponse) {
        (InterceptResult::PASSTHROUGH, packet)
    }

    pub async fn query_pong(packet: s2c::QueryPong, _reader: &OwnedReadHalf) -> (InterceptResult, s2c::QueryPong) {
        (InterceptResult::PASSTHROUGH, packet)
    }
}

pub fn ip_cache(reader: &OwnedReadHalf) {
    let ip = reader.peer_addr().unwrap().ip().to_string();
    log!("Saving IP", &reader);
    thread::spawn(move || {
        if VIGILANT_CONFIG.guardian.ping_protection.active {
            RUNTIME.spawn(async move {
                let timestamp = chrono::Utc::now().timestamp();

                if let Some(_) = { IP_CACHE.lock().await.values().find(|&v| v == &ip) } {
                    return;
                }

                drop(IP_CACHE.lock().await.insert(timestamp, ip));

                thread::sleep(Duration::from_secs(VIGILANT_CONFIG.guardian.ping_protection.reset_interval));

                IP_CACHE.lock().await.remove(&timestamp);
            });
        }
    });
}

pub fn ip_forward(packet: &mut c2s::Handshake, reader: &OwnedReadHalf) {
    if VIGILANT_CONFIG.proxy.forwarder.ip_forward {
        packet.server_address = format!("{addr}|{player_addr}", addr = packet.server_address, player_addr = reader.peer_addr().unwrap().ip().to_string());
    }
}

// pub fn query_response(packet: &mut c2s::QueryResponse) {
//     if !VIGILANT_CONFIG.proxy.forwarder.motd_forward {
//         let json = serde_json::from_str::<Value>(&packet.json).unwrap();
//         let source = packet.json.to_string();

//         let description = Text::from(VIGILANT_LANG.server_motd.clone());
//         let description_from = serde_json::to_string(&json["description"]).unwrap();
//         let description_to = serde_json::to_string(&description).unwrap();

//         let version_name = &VIGILANT_LANG.server_version_name;
//         let version_from = serde_json::to_string(&json["version"]["name"]).unwrap();
//         let version_to = serde_json::to_string(&version_name).unwrap();

//         let source = source.replace(&description_from, &description_to).replace(&version_from, &version_to);

//         packet.json = source;
//     }
// }

pub async fn vpn_filter(reader: &OwnedReadHalf) -> Option<BytesMut> {
    let ip = reader.peer_addr().unwrap().ip().to_string();

    if VIGILANT_CONFIG.guardian.vpn_filter.active {
        if ip_blacklisted(ip).await {
            reject!(VIGILANT_LANG.player_ip_blacklisted_kick.clone(), "Using VPN/Proxy", reader);
        }
    }

    None
}

pub async fn concurrency_filter(reader: &OwnedReadHalf) -> Option<BytesMut> {
    let ip = reader.peer_addr().unwrap().ip().to_string();

    if VIGILANT_CONFIG.guardian.ip_connection_limit.active {
        if CONNECTIONS.lock().await.get(&ip).unwrap() >= &VIGILANT_CONFIG.guardian.ip_connection_limit.limit {
            reject!(VIGILANT_LANG.player_connection_more_kick.clone(), "IP Connection limit is exceeded", reader);
        }
    }

    None
}

pub async fn ping_filter(reader: &OwnedReadHalf) -> Option<BytesMut> {
    let ip = reader.peer_addr().unwrap().ip().to_string();

    if VIGILANT_CONFIG.guardian.ping_protection.active {
        if let None = IP_CACHE.lock().await.values().find(|&v| v == &ip) {
            reject!(VIGILANT_LANG.player_ping_not_cached_kick.clone(), "Player have not pinged", reader);
        }
    }

    None
}
