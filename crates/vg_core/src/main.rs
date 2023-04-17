mod interceptor;
pub mod packet;
pub mod macros;
pub mod guardian;
mod logger;
mod file;

use std::{net::{SocketAddr, ToSocketAddrs}, sync::{Arc}, collections::HashMap};

use atomic_float::AtomicF64;
use log::{info, trace};
use logger::terminal;
use once_cell::sync::Lazy;
use packet::*;

use tokio::{net::{TcpStream, TcpListener}, sync::Mutex, runtime::Runtime};
use vg_macro::random_id;

use crate::interceptor::{front, middle};

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
const IP_FORWARD: bool = true;
const VPN_PROTECTION: bool = true;

async fn proxy(client: TcpStream, server: TcpStream, state: PacketState) -> Result<(), Box<dyn std::error::Error>> {
    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut server_reader, mut server_writer) = server.into_split();

    let _state = Arc::new(Mutex::new(state));
    let packet_stage = Arc::new(Mutex::new(PacketStage::C2sHandshake));

    let (mut buf, mut buf1) = ([0u8; 4096], [0u8; 4096]);

    loop {
        if front::Interceptor::init(&mut client_reader, &mut client_writer, packet_stage.clone()).await.intercepted {
            break;
        }

        let client_to_server = middle::Interceptor::init(&mut client_reader, &mut server_writer, &mut buf, packet_stage.clone());
        let server_to_client = middle::Interceptor::init(&mut server_reader, &mut client_writer, &mut buf1, packet_stage.clone());

        tokio::select! {
            (len, _) = client_to_server => {
                if len == 0 { break; }
            },
            (len, _) = server_to_client => {
                if len == 0 { break; }
            }
        }
    }

    Ok(())
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
            info!("{}", colorizer!("[/c(dark_blue){address}c(reset)] New Connection"));

            addr = address.ip().to_string();

            *CONNECTIONS.lock().await.entry(addr.clone()).or_insert(0) += 1;
            
            socket
        } else { panic!("Failed to accept a new connection") };
        
        RUNTIME.spawn(async move {
            if let Ok(server_socket) = TcpStream::connect(server_address).await {
                let current_state = PacketState::Handshake;
    
                proxy(client_socket, server_socket, current_state).await.unwrap();
            }

            trace!("Dropping connection with ip {address}", address = addr);
            CONNECTIONS.lock().await.entry(addr).and_modify(|v| *v -= 1);
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy_address = "127.0.0.1:25565";
    let server_address = "127.0.0.1:25577";

    terminal::setup().expect("Failed to setup interactive terminal!");

    info!("{}", colorizer!("Loading VigilantGuard build ({})", BUILD_ID));

    let proxy_address = proxy_address.to_socket_addrs()?.next().unwrap();
    let server_address = server_address.to_socket_addrs()?.next().unwrap();

    accept_loop(proxy_address, server_address).await;
    Ok(())
}
