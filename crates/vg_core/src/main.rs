mod interceptor;
pub mod packet;
pub mod macros;
pub mod guardian;
mod logger;
mod file;

use std::{net::{SocketAddr, ToSocketAddrs}, collections::HashMap};

use atomic_float::AtomicF64;
use interceptor::{interceptor::{Interceptor, InterceptResult}, gate};
use log::{info, trace};
use logger::terminal;
use once_cell::sync::Lazy;
use packet::*;

use tokio::{net::{TcpStream, TcpListener, tcp::{OwnedReadHalf, OwnedWriteHalf}}, sync::{Mutex}, runtime::Runtime, io::{AsyncReadExt, AsyncWriteExt}};
use valence_protocol::{encoder::PacketEncoder, decoder::PacketDecoder, bytes::BytesMut, packet::{c2s::{handshake::{handshake::NextState}, status::{QueryRequestC2s, QueryPingC2s}, login::LoginHelloC2s}, s2c::{status::{QueryPongS2c}}}};
use vg_macro::{random_id};

#[macro_use]
extern crate lazy_static;

static mut TOTAL_DOWNLOAD: AtomicF64 = AtomicF64::new(0.0);
static mut TOTAL_UPLOAD: AtomicF64 = AtomicF64::new(0.0);

static RUNTIME: Lazy<Runtime> = Lazy::new(|| { 
    tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("proxy").build().expect("Failed to create a new runtime") 
});

lazy_static! {
    static ref IP_CACHE: Mutex<HashMap<i64, String>> = Mutex::new(HashMap::new());
    static ref CONNECTIONS: Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
}

random_id!("BUILD_ID");

const PING_PROTECTION: bool = true;
const IP_CONCURRENT_LIMIT: usize = 1;
const PING_FORWARD: bool = false;
const IP_FORWARD: bool = true;
const VPN_PROTECTION: bool = true;

async fn proxy(client: TcpStream, server: TcpStream) -> anyhow::Result<()> {
    let (client_reader, client_writer) = client.into_split();
    let (server_reader, server_writer) = server.into_split();

    let c2s = Mutex::new(Interceptor {
        direction: PacketDirection::C2S,
        reader: Some(client_reader),
        writer: Some(server_writer),
        encoder: PacketEncoder::new(),
        decoder: PacketDecoder::new(),
        frame: BytesMut::new(),
        other: None,
    });

    let s2c = Mutex::new(Interceptor {
        direction: PacketDirection::S2C,
        reader: Some(server_reader),
        writer: Some(client_writer),
        encoder: PacketEncoder::new(),
        decoder: PacketDecoder::new(),
        frame: BytesMut::new(),
        other: None,
    });

    c2s.lock().await.other = Some(&s2c);
    s2c.lock().await.other = Some(&c2s);

    let next = make_gatekeeper!(c2s; HandshakeC2sOwn; |mut packet, reader| async move {
        gate::ip_forward(&mut packet, reader);

        (InterceptResult::PASSTHROUGH, packet)
    }).next_state;

    match next {
        NextState::Status => {
            make_gatekeeper!(c2s; QueryRequestC2s; |packet, reader| async move {
                gate::ip_cache(reader);

                (InterceptResult::PASSTHROUGH, packet)
            });

            make_gatekeeper!(s2c; QueryResponseS2cOwn; |mut packet, _| async move {
                gate::query_response(&mut packet);

                (InterceptResult::PASSTHROUGH, packet)
            });

            make_gatekeeper!(c2s; QueryPingC2s; |packet, _| async move {
                if PING_FORWARD {
                    (InterceptResult::PASSTHROUGH, packet)
                } else {
                    (InterceptResult::RETURN(None), packet)
                }
            });

            if PING_FORWARD {
                make_gatekeeper!(s2c; QueryPongS2c);
            }
        },
        NextState::Login => {
            make_gatekeeper!(c2s; LoginHelloC2s; |packet, reader| async move {

                if let Some(bytes) = gate::concurrency_filter(reader) {
                    return (InterceptResult::RETURN(Some(bytes)), packet);
                }

                if let Some(bytes) = gate::ping_filter(reader).await {
                    return (InterceptResult::RETURN(Some(bytes)), packet);
                }

                if let Some(bytes) = gate::vpn_filter(reader).await {
                    return (InterceptResult::RETURN(Some(bytes)), packet);
                }

                (InterceptResult::PASSTHROUGH, packet)
            });

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

        write.write_all(bytes).await?;
    }
}

async fn accept_loop(proxy_address: SocketAddr, server_address: SocketAddr) {
    let listener = 
        if let Ok(listener) = TcpListener::bind(proxy_address).await 
        {
            info!("{}", colorizer!("c(on_red) VigilantGuard c(reset) is started at c(on_blue) {} ", proxy_address.to_string()));
            listener
        } else { panic!("Failed to start the proxy server") };

    loop {
        let addr;

        let client_socket = if let Ok((socket, address)) = listener.accept().await {
            info!("{}", colorizer!("[/c(dark_blue){address}c(reset)] New connection"));

            addr = address.ip().to_string();

            *CONNECTIONS.lock().await.entry(addr.clone()).or_insert(0) += 1;
            
            socket
        } else { panic!("Failed to accept a new connection") };
        
        RUNTIME.spawn(async move {
            if let Ok(server_socket) = TcpStream::connect(server_address).await {
                server_socket.set_nodelay(true).unwrap();
    
                proxy(client_socket, server_socket).await.unwrap();
            }

            trace!("Dropping connection with ip {address}", address = addr);
            CONNECTIONS.lock().await.entry(addr).and_modify(|v| *v -= 1);
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy_address = "159.65.140.219:25565";
    let server_address = "pn-32gb.rapstore.online:25565";

    terminal::setup().expect("Failed to setup interactive terminal!");

    info!("{}", colorizer!("Loading VigilantGuard build ({}-{}-{})", env!("VERGEN_GIT_BRANCH"), env!("VERGEN_GIT_DESCRIBE"), env!("VERGEN_BUILD_DATE")));

    let proxy_address = proxy_address.to_socket_addrs()?.next().unwrap();
    let server_address = server_address.to_socket_addrs()?.next().unwrap();

    accept_loop(proxy_address, server_address).await;
    Ok(())
}

// TODO: Add config system
