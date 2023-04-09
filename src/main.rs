use std::{net::{SocketAddr, ToSocketAddrs}, thread, time::Duration, sync::atomic::Ordering};

use atomic_float::AtomicF64;
use tokio::{net::{TcpStream, TcpListener, tcp::{OwnedReadHalf, OwnedWriteHalf}}, io::{AsyncWriteExt, AsyncReadExt}};

static mut TOTAL_DOWNLOAD: AtomicF64 = AtomicF64::new(0.0);
static mut TOTAL_UPLOAD: AtomicF64 = AtomicF64::new(0.0);

async fn intercept_copy(reader: &mut OwnedReadHalf, writer: &mut OwnedWriteHalf) -> usize {
    let mut buffer = [0u8; 1024];

    let read_len = reader.read(&mut buffer).await.unwrap();
    let read_buf = &buffer[..read_len];

    let write_len = writer.write(read_buf).await.unwrap();

    unsafe {
        TOTAL_DOWNLOAD.fetch_add(read_len as f64, Ordering::Relaxed);
        TOTAL_UPLOAD.fetch_add(write_len as f64, Ordering::Relaxed);
    }

    read_len
}

async fn proxy(client: TcpStream, server: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut server_reader, mut server_writer) = server.into_split();

    loop {
        let client_to_server = intercept_copy(&mut client_reader, &mut server_writer);
        let server_to_client = intercept_copy(&mut server_reader, &mut client_writer);

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

    loop {
        let client_socket = if let Ok((socket, address)) = listener.accept().await {
            println!("Started a new connection with ip {address}");
            socket
        } else { panic!("Failed to accept a new connection") };

        rt.spawn(async move {
            let server_socket = TcpStream::connect(server_address).await.expect("Failed to connect to the server");

            if let Err(e) = proxy(client_socket, server_socket).await {
                eprintln!("Error: {}", e);
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proxy_address = "127.0.0.1:25565";
    let server_address = "159.65.140.219:25505";

    let proxy_address = proxy_address.to_socket_addrs()?.next().unwrap();
    let server_address = server_address.to_socket_addrs()?.next().unwrap();

    thread::spawn(move || {
        loop {
            // unsafe {
            //     println!("Total Download: {total}MB", total = TOTAL_DOWNLOAD.load(Ordering::Relaxed) / 1e+6);
            //     println!("Total Upload: {total}MB", total = TOTAL_UPLOAD.load(Ordering::Relaxed) / 1e+6);
            // }

            thread::sleep(Duration::from_secs(1));
        }
    });

    accept_loop(proxy_address, server_address).await;
    Ok(())
}
