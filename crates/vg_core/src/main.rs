mod file;
pub mod guardian;
mod interceptor;
mod logger;
pub mod macros;
pub mod packet;

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use atomic_float::AtomicF64;
use interceptor::gate;
use interceptor::interceptor::{InterceptResult, Interceptor};
use log::info;
use logger::terminal;
use once_cell::sync::Lazy;
use packet::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;
use valence_protocol::bytes::BytesMut;
use valence_protocol::decoder::PacketDecoder;
use valence_protocol::encoder::PacketEncoder;
use valence_protocol::packet::c2s::handshake::handshake::NextState;
use valence_protocol::packet::c2s::login::LoginHelloC2s;
use valence_protocol::packet::c2s::status::{QueryPingC2s, QueryRequestC2s};
use valence_protocol::packet::s2c::login::LoginDisconnectS2c;
use valence_protocol::packet::s2c::status::QueryPongS2c;
use valence_protocol::text::Text;
use vg_macro::make_gatekeeper;

use crate::file::{VIGILANT_CONFIG, VIGILANT_LANG};

#[macro_use]
extern crate lazy_static;

static mut TOTAL_DOWNLOAD: AtomicF64 = AtomicF64::new(0.0);
static mut TOTAL_UPLOAD: AtomicF64 = AtomicF64::new(0.0);

static SERVER_ALIVE: AtomicBool = AtomicBool::new(false);

static RUNTIME: Lazy<Runtime> = Lazy::new(|| tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("proxy").build().expect("Failed to create a new runtime"));

lazy_static! {
    static ref IP_CACHE: Mutex<HashMap<i64, String>> = Mutex::new(HashMap::new());
    static ref CONNECTIONS: Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
    static ref PLAYERS: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
}

async fn proxy(client: TcpStream, server: TcpStream) -> anyhow::Result<()> {
    let (client_reader, client_writer) = client.into_split();
    let (server_reader, server_writer) = server.into_split();

    let c2s = Mutex::new(Interceptor { direction: PacketDirection::C2S, reader: Some(client_reader), writer: Some(server_writer), encoder: PacketEncoder::new(), decoder: PacketDecoder::new(), frame: BytesMut::new(), other: None });
    let s2c = Mutex::new(Interceptor { direction: PacketDirection::S2C, reader: Some(server_reader), writer: Some(client_writer), encoder: PacketEncoder::new(), decoder: PacketDecoder::new(), frame: BytesMut::new(), other: None });

    c2s.lock().await.other = Some(&s2c);
    s2c.lock().await.other = Some(&c2s);
    
    let next = make_gatekeeper!(c2s, Handshake).next_state;

    if !SERVER_ALIVE.load(Ordering::Relaxed) {
        match next {
            NextState::Status => {
                make_gatekeeper!(c2s, QueryRequest);
            },
            NextState::Login => {
                make_gatekeeper!(c2s, LoginHello);
            },
        }

        return Ok(());
    }

    match next {
        NextState::Status => {
            make_gatekeeper!(c2s, QueryRequest);
            make_gatekeeper!(s2c, QueryResponse);
            make_gatekeeper!(c2s, QueryPing);
            make_gatekeeper!(s2c, QueryPong);
        },
        NextState::Login => {
            make_gatekeeper!(c2s, LoginHello);

            let mut c2s = c2s.lock().await;
            let mut s2c = s2c.lock().await;

            return tokio::select! {
                c2s_res = passthrough(c2s.reader.take().unwrap(), c2s.writer.take().unwrap()) => c2s_res,
                s2c_res = passthrough(s2c.reader.take().unwrap(), s2c.writer.take().unwrap()) => s2c_res,
            };
        },
    }

    return Ok(());
}

async fn passthrough(mut read: OwnedReadHalf, mut write: OwnedWriteHalf) -> anyhow::Result<()> {
    let mut buf = Box::new([0u8; 8192]);
    loop {
        let bytes_read = read.read(buf.as_mut_slice()).await?;
        let bytes = &mut buf[..bytes_read];

        if bytes.is_empty() {
            break Ok(());
        }

        let bytes_write = write.write(bytes).await?;

        unsafe {
            TOTAL_DOWNLOAD.fetch_add(bytes_read as f64, std::sync::atomic::Ordering::Relaxed);
            TOTAL_UPLOAD.fetch_add(bytes_write as f64, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

async fn accept_loop(proxy_address: SocketAddr, server_address: SocketAddr) {
    let listener = if let Ok(listener) = TcpListener::bind(proxy_address).await {
        info!("{}", colorizer!("c(on_red) VigilantGuard c(reset) is started at c(on_blue) {} ", proxy_address.to_string()));
        listener
    } else {
        panic!("Failed to start the proxy server")
    };

    loop {
        let addr;

        let client_socket = if let Ok((socket, address)) = listener.accept().await {
            info!("{}", colorizer!("[/c(dark_blue){address}c(reset)] Open connection"));

            addr = address;

            *CONNECTIONS.lock().await.entry(addr.clone().ip().to_string()).or_insert(0) += 1;

            socket
        } else {
            panic!("Failed to accept a new connection")
        };

        RUNTIME.spawn(async move {
            let server = TcpStream::connect(server_address).await;
            match server {
                Ok(server_socket) => {
                    server_socket.set_nodelay(true).unwrap();
                    SERVER_ALIVE.store(true, Ordering::Relaxed);

                    if let Err(err) = proxy(client_socket, server_socket).await {
                        log::error!("{}", colorizer!("[/c(dark_blue){}c(reset)] {}", addr.to_string(), err.to_string()));
                    }
                }
                Err(err) => {
                    if let ErrorKind::ConnectionRefused = err.kind() {
                        SERVER_ALIVE.store(false, Ordering::Relaxed);
                        let dummy_server = TcpListener::bind("127.0.0.1:0").await.unwrap();
                        let dummy_socket = TcpStream::connect(dummy_server.local_addr().unwrap()).await.unwrap();
                        if let Err(err) = proxy(client_socket, dummy_socket).await {
                            log::error!("{}", colorizer!("[/c(dark_blue){}c(reset)] {}", addr.to_string(), err.to_string()));
                        }
                    }
                }
            }

            PLAYERS.lock().await.remove(&addr.to_string());
            info!("{}", colorizer!("[/c(dark_blue){}c(reset)] Close connection", addr.to_string()));
            CONNECTIONS.lock().await.entry(addr.ip().to_string()).and_modify(|v| *v -= 1);
        });
    }
}

fn config_warn() {
    // if !VIGILANT_CONFIG.proxy.forwarder.motd_forward {
    //     log::warn!("{}", colorizer!("c(on_yellow) MOTD INTERCEPT IS WIP!! "));
    //     log::warn!("{}", colorizer!("c(on_yellow) DO NOT SET THIS TO FALSE IF THIS IS IN PRODUCTION!!! "));
    // }

    if !VIGILANT_CONFIG.proxy.forwarder.ip_forward {
        log::warn!("{}", colorizer!("c(on_yellow) PLEASE TURN ON IP FORWARD!!! "));
        log::warn!("{}", colorizer!("c(on_yellow) UNLESS YOU KNOW WHAT YOU'RE DOING! "));
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy_address = &format!("{}:{}", VIGILANT_CONFIG.proxy.ip, VIGILANT_CONFIG.proxy.port);
    let server_address = &format!("{}:{}", VIGILANT_CONFIG.server.ip, VIGILANT_CONFIG.server.port);

    terminal::setup().expect("Failed to setup interactive terminal!");

    info!("{}", colorizer!("Loading VigilantGuard build ({}-{}-{})", env!("VERGEN_GIT_BRANCH"), env!("VERGEN_GIT_DESCRIBE"), env!("VERGEN_BUILD_DATE")));

    let _ = &VIGILANT_LANG.server_offline_kick; // Preload the lang file to memory

    config_warn();

    let proxy_address = proxy_address.to_socket_addrs()?.next().unwrap();
    let server_address = server_address.to_socket_addrs()?.next().unwrap();

    accept_loop(proxy_address, server_address).await;
    Ok(())
}
