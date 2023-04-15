mod interceptor;
pub mod packet;
pub mod macros;

use std::{net::{SocketAddr, ToSocketAddrs}, thread, time::Duration, sync::{Arc, atomic::Ordering}, collections::HashMap, io::{Write, BufWriter}};

use atomic_float::AtomicF64;
use interceptor::MiddleInterceptor;
use log::{LevelFilter, info, trace, error};
use packet::*;

use log4rs::{*, append::{Append}, config::{Appender, Root}, encode::{pattern::PatternEncoder, writer::{simple::SimpleWriter}, Encode}};
use rustyline::{DefaultEditor, ExternalPrinter, error::ReadlineError};
use tokio::{net::{TcpStream, TcpListener}, io::{AsyncWriteExt, AsyncReadExt}, sync::Mutex, runtime::Runtime};

use crate::interceptor::FrontInterceptor;

static mut TOTAL_DOWNLOAD: AtomicF64 = AtomicF64::new(0.0);
static mut TOTAL_UPLOAD: AtomicF64 = AtomicF64::new(0.0);

const PING_PROTECTION: bool = true;
const IP_CONCURRENT_LIMIT: usize = 1;
const IP_FORWARD: bool = true;

async fn proxy(client: TcpStream, server: TcpStream, state: PacketState, ip_cache: Arc<Mutex<HashMap<i64, String>>>, connections: Arc<Mutex<HashMap<String, usize>>>) -> Result<(), Box<dyn std::error::Error>> {
    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut server_reader, mut server_writer) = server.into_split();

    let state = Arc::new(Mutex::new(state));
    let packet_stage = Arc::new(Mutex::new(PacketStage::C2sHandshake));

    let (mut buf, mut buf1) = ([0u8; 4096], [0u8; 4096]);

    loop {
        if FrontInterceptor::init(&mut client_reader, &mut client_writer, packet_stage.clone(), connections.clone(), ip_cache.clone()).await.intercepted {
            break;
        }

        let client_to_server = MiddleInterceptor::init(&mut client_reader, &mut server_writer, &mut buf, connections.clone(), packet_stage.clone(), ip_cache.clone());
        let server_to_client = MiddleInterceptor::init(&mut server_reader, &mut client_writer, &mut buf1, connections.clone(), packet_stage.clone(), ip_cache.clone());

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
    
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("proxy").build().expect("Failed to create a new runtime");

    let ip_cache: Arc<Mutex<HashMap<i64, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let connections: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let connections = connections.clone();
        let ip_cache = ip_cache.clone();
        let mut addr = String::new();

        let client_socket = if let Ok((socket, address)) = listener.accept().await {
            info!("{}", colorizer!("[/c(dark_blue){address}c(reset)] New Connection"));
            std::io::stdout().flush().unwrap();

            addr = address.ip().to_string();

            *(connections.lock().await).entry(addr.clone()).or_insert(0) += 1;
            
            socket
        } else { panic!("Failed to accept a new connection") };
        
        rt.spawn(async move {
            if let Ok(server_socket) = TcpStream::connect(server_address).await {
                let current_state = PacketState::Handshake;
    
                unsafe { proxy(client_socket, server_socket, current_state, ip_cache, connections.clone()).await.unwrap_unchecked() }
            }

            trace!("Dropping connection with ip {address}", address = addr);
            connections.lock().await.entry(addr).and_modify(|v| *v -= 1);
        });
    }
}

struct LogAppender<T: FnMut(String) + Sync + Send + 'static> {
    printer: std::sync::Mutex<T>,
    encoder: Box<dyn Encode>
}

impl<F: FnMut(String) + Sync + Send + 'static> Append for LogAppender<F> {
    fn append(&self, record: &log::Record) -> anyhow::Result<()> {
        let mut writer = SimpleWriter(BufWriter::new(Vec::new()));
        self.encoder.encode(&mut writer, record).unwrap();
        let str = String::from_utf8_lossy(writer.0.buffer());
        let color = match record.level() {
            log::Level::Error => "\x1b[1;31m",
            log::Level::Warn => "\x1b[0;33m",
            log::Level::Info => "\x1b[1;32m",
            _ => ""
        };
        (self.printer.lock().unwrap())(str.to_string().replace(record.level().as_str(), format!("{}{}\x1b[0m", color, record.level().as_str()).as_str()));
        Ok(())
    }

    fn flush(&self) {
        todo!()
    }
}

impl<F: FnMut(String) + Sync + Send + 'static> std::fmt::Debug for LogAppender<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogAppender").field("encoder", &self.encoder).finish()
    }
}

fn setup_terminal() -> Result<(), ()> {
    let mut rl = DefaultEditor::new().unwrap();
    let mut printer = rl.create_external_printer().unwrap();

    thread::Builder::new().name("command".to_string()).spawn(move || {
        loop {
            let line = rl.readline("> ");
            match line {
                Ok(line) => {
                    rl.add_history_entry(&line).unwrap();

                    match line.as_str() {
                        "stop" => {
                            info!("{}", colorizer!("c(bright_red)Stopping"));
                            std::process::exit(1);
                        }
                        "usage" => {
                            unsafe {
                                info!("\x1b[1;32;42m ⬇ {}MB \x1b[0m\x1b[1;33;43m ⬆ {}MB ", TOTAL_DOWNLOAD.load(Ordering::Relaxed) / 1e+6, TOTAL_UPLOAD.load(Ordering::Relaxed) / 1e+6);
                            }
                        }
                        _ => {
                            if line.len() > 0 {
                                info!("Unknown command {:?}", line);
                            }
                        }
                    }
                },
                Err(err) => {
                    if let ReadlineError::Interrupted = err {
                        std::process::exit(1);
                    }

                    error!("{}", colorizer!("c(bright_red){}", err.to_string()));
                },
            }
        }
    }).unwrap();

    let patt = "[{d(%H:%M:%S)}] {([{T}/{h({l})}]):<12}: {m}\x1b[0m\n";

    // let stdout = ConsoleAppender::builder().encoder(Box::new(PatternEncoder::new(patt))).build();
    let stdout = LogAppender {
        printer: std::sync::Mutex::new(move |v| { printer.print(v).unwrap() }),
        encoder: Box::new(PatternEncoder::new(patt))
    };

    let config = Config::builder()
        .appenders([
            Appender::builder().build("stdout", Box::new(stdout)),
        ])
        .build(Root::builder().appenders(["stdout"]).build(LevelFilter::Info)).unwrap();

    let _handle = log4rs::init_config(config).unwrap();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy_address = "127.0.0.1:25565";
    let server_address = "127.0.0.1:25577";

    setup_terminal().unwrap();

    let proxy_address = proxy_address.to_socket_addrs()?.next().unwrap();
    let server_address = server_address.to_socket_addrs()?.next().unwrap();

    // thread::spawn(move || {
    //     info!("other thred");
    //     loop {
    //         unsafe {
    //             // println!("Total Download: {total}MB", total = TOTAL_DOWNLOAD.load(Ordering::Relaxed) / 1e+6);
    //             // println!("Total Upload: {total}MB", total = TOTAL_UPLOAD.load(Ordering::Relaxed) / 1e+6);
    //         }

    //         thread::sleep(Duration::from_secs(1));
    //     }
    // });

    accept_loop(proxy_address, server_address).await;
    Ok(())
}
