#![feature(forget_unsized)]

mod interceptor;
pub mod packet;
pub mod macros;

use std::{net::{SocketAddr, ToSocketAddrs}, thread, time::Duration, sync::{atomic::{Ordering, AtomicUsize}, Arc}, collections::HashMap};

use atomic_float::AtomicF64;
use interceptor::MiddleInterceptor;
use packet::*;
use tokio::{net::{TcpStream, TcpListener, tcp::{OwnedReadHalf, OwnedWriteHalf}}, io::{AsyncWriteExt, AsyncReadExt}, sync::Mutex, runtime::Runtime};
use valence_protocol::{Decode, Encode, packet::{s2c::login::{LoginSuccessS2c, LoginHelloS2c}, S2cLoginPacket}, Packet};

use crate::interceptor::FrontInterceptor;

static mut TOTAL_DOWNLOAD: AtomicF64 = AtomicF64::new(0.0);
static mut TOTAL_UPLOAD: AtomicF64 = AtomicF64::new(0.0);

const PING_PROTECTION: bool = true;
const IP_CONCURRENT_LIMIT: usize = 1;
const IP_FORWARD: bool = true;

async fn middle_intercept(
    reader: &mut OwnedReadHalf, 
    writer: &mut OwnedWriteHalf, 
    packet_direction: PacketDirection, 
    packet_state_arc: Arc<Mutex<PacketState>>, 
    ip_cache: Arc<Mutex<HashMap<i64, String>>>, 
    connections: Arc<Mutex<HashMap<String, usize>>>,
    packet_stage: Arc<Mutex<PacketStage>>
) -> usize {
    let mut buffer = [0u8; 4096];

    let read_len = reader.read(&mut buffer).await.unwrap_or(0);
    if read_len == 0 { return 0; }
    
    let packet_state = packet_state_arc.lock().await.clone();
    let mut middle_interceptor = MiddleInterceptor::new(reader, writer, &mut buffer, read_len, connections);
    
    match packet_direction {
        PacketDirection::C2S => {
            match &packet_state {
                PacketState::Handshake => {
                    PacketHeader::decode(&mut middle_interceptor.array_buffer).expect("Failed to decode packet header");

                    middle_interceptor.c2s_handshake(ip_cache.clone(), packet_state_arc.clone()).await;
                    *packet_stage.lock().await = PacketStage::S2cQueryResponse;
                }
                PacketState::Status => {
                    if let Ok(_) = C2sPingRequest::decode(&mut &middle_interceptor.raw_buffer[..]) {
                        middle_interceptor.passthrough();
                        *packet_stage.lock().await = PacketStage::S2cPingResponse;
                    }
                }
                PacketState::Login => {
                    middle_interceptor.passthrough();
                }
                _ => { 
                    middle_interceptor.passthrough();
                }
            }
        }
        PacketDirection::S2C => {
            match &packet_state {
                PacketState::Status => {
                    let packet_header = PacketHeader::decode(&mut middle_interceptor.array_buffer).expect("Failed to decode packet header");
                    match packet_header.packet_id.0 {
                        0x00 => {
                            middle_interceptor.s2c_status().await;
                            *packet_stage.lock().await = PacketStage::C2sPingRequest;
                        }
                        _ => {
                            middle_interceptor.passthrough();
                        }
                    }
                }
                PacketState::Login => {
                    // TODO: possibly intercept plugin message?
                    middle_interceptor.s2c_login(ip_cache).await;

                    // printhex!(middle_interceptor.array_buffer);

                    // if let Ok(packet) = LoginHelloS2c::decode(&mut middle_interceptor.array_buffer) {
                    //     dbg!(packet);
                    // }

                    *packet_state_arc.lock().await = PacketState::Play;
                }
                _ => {
                    middle_interceptor.passthrough();
                }
            }
        },
    }

    let response = if middle_interceptor.passthrough { &middle_interceptor.vector_buffer } else { &middle_interceptor.response_buffer };

    let write_len = middle_interceptor.writer.write(response).await.unwrap_or(0);

    unsafe {
        TOTAL_DOWNLOAD.fetch_add(read_len as f64, Ordering::Relaxed);
        TOTAL_UPLOAD.fetch_add(write_len as f64, Ordering::Relaxed);
    }

    read_len
}

async fn front_intercept(reader: &mut OwnedReadHalf, writer: &mut OwnedWriteHalf, packet_stage: Arc<Mutex<PacketStage>>) -> Result<(), ()> {
    let mut front_interceptor = FrontInterceptor::new(reader, writer);

    match *packet_stage.lock().await {
        PacketStage::C2sPingRequest => {
            front_interceptor.reply_ping().await;
        }
        _ => {  }
    }

    if front_interceptor.intercepted { return Ok(()) } else { return Err(()) }
}

async fn proxy(client: TcpStream, server: TcpStream, state: PacketState, ip_cache: Arc<Mutex<HashMap<i64, String>>>, connections: Arc<Mutex<HashMap<String, usize>>>) -> Result<(), Box<dyn std::error::Error>> {
    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut server_reader, mut server_writer) = server.into_split();

    let state = Arc::new(Mutex::new(state));
    let packet_stage = Arc::new(Mutex::new(PacketStage::C2sHandshake));

    loop {
        if let Ok(_) = front_intercept(&mut client_reader, &mut client_writer, packet_stage.clone()).await {
            break;
        }

        let client_to_server = middle_intercept(&mut client_reader, &mut server_writer, PacketDirection::C2S, state.clone(), ip_cache.clone(), connections.clone(), packet_stage.clone());
        let server_to_client = middle_intercept(&mut server_reader, &mut client_writer, PacketDirection::S2C, state.clone(), ip_cache.clone(), connections.clone(), packet_stage.clone());

        tokio::select! {
            len = client_to_server => {
                if len == 0 { break }
            },
            len = server_to_client => {
                if len == 0 { break }
            }
        }
    }

    Ok(())
}

async fn accept_loop(proxy_address: SocketAddr, server_address: SocketAddr) {
    let listener = 
        if let Ok(listener) = TcpListener::bind(proxy_address).await 
        {
            println!("VigilantGuard is started at {address}", address = proxy_address.to_string());
            listener
        } else { panic!("Failed to start the proxy server") };

    let rt = tokio::runtime::Runtime::new().expect("Failed to create a new runtime");

    let ip_cache: Arc<Mutex<HashMap<i64, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let connections: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let connections = connections.clone();
        let ip_cache = ip_cache.clone();
        let mut addr = String::new();

        let client_socket = if let Ok((socket, address)) = listener.accept().await {
            println!("Started a new connection with ip {address}");

            addr = address.ip().to_string();

            *(connections.lock().await).entry(addr.clone()).or_insert(0) += 1;
            
            socket
        } else { panic!("Failed to accept a new connection") };
        
        rt.spawn(async move {
            if let Ok(server_socket) = TcpStream::connect(server_address).await {
                let current_state = PacketState::Handshake;
    
                unsafe { proxy(client_socket, server_socket, current_state, ip_cache, connections.clone()).await.unwrap_unchecked() }
            }

            println!("Dropping connection with ip {address}", address = addr);
            connections.lock().await.entry(addr).and_modify(|v| *v -= 1);
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy_address = "127.0.0.1:25565";
    let server_address = "127.0.0.1:25577";

    let proxy_address = proxy_address.to_socket_addrs()?.next().unwrap();
    let server_address = server_address.to_socket_addrs()?.next().unwrap();

    thread::spawn(move || {
        loop {
            unsafe {
                // println!("Total Download: {total}MB", total = TOTAL_DOWNLOAD.load(Ordering::Relaxed) / 1e+6);
                // println!("Total Upload: {total}MB", total = TOTAL_UPLOAD.load(Ordering::Relaxed) / 1e+6);
            }

            thread::sleep(Duration::from_secs(1));
        }
    });

    accept_loop(proxy_address, server_address).await;
    Ok(())
}
