use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{BufMut, BytesMut};
use reqwest::{Body, Url};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::{
    watch::{Receiver, Sender},
    Barrier,
};

pub async fn request_tcp_ping(host: &SocketAddr) -> Result<u128, Box<dyn Error + Send + Sync>> {
    let now = Instant::now();
    let _stream = TcpStream::connect(host).await?;
    let used = now.elapsed().as_micros();
    Ok(used)
}

pub async fn request_tcp_download(
    addr: SocketAddr,
    barrier: Arc<Barrier>,
    stop_rx: &mut Receiver<&str>,
    counter_tx: Sender<u128>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let mut count = 0;
    let data_size: u64 = 15_000_000_000;
    let command = format!("DOWNLOAD {}\n", data_size);
    let mut buff: [u8; 16384] = [0; 16384];

    let mut stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;
    let _r = barrier.wait().await;
    stream.write_all(command.as_bytes()).await?;

    while *stop_rx.borrow() != "stop" {
        stream.readable().await?;
        match stream.try_read(&mut buff) {
            Ok(n) => {
                count += n as u128;
                let _r = counter_tx.send(count);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    Ok(true)
}

pub async fn request_tcp_upload(
    addr: SocketAddr,
    barrier: Arc<Barrier>,
    stop_rx: Receiver<&str>,
    counter_tx: Sender<u128>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let mut count = 0;
    let data_size: u64 = 15_000_000_000;
    let data = "23456789ABCDEFGHIJKLMNOPQRSTUVWX".repeat(1_000_000);
    let data = data.as_bytes();

    let command = format!("UPLOAD {} 0\n", data_size);

    let mut stream = TcpStream::connect(addr).await?;
    stream.set_nodelay(true)?;
    let _r = barrier.wait().await;
    stream.write_all(command.as_bytes()).await?;

    while *stop_rx.borrow() != "stop" {
        stream.writable().await?;
        match stream.try_write(data) {
            Ok(n) => {
                count += n as u128;
                let _r = counter_tx.send(count);
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    Ok(true)
}

pub async fn request_http_download(
    url: Url,
    addr: SocketAddr,
    barrier: Arc<Barrier>,
    stop_rx: &mut Receiver<&str>,
    counter_tx: Sender<u128>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let mut count = 0;

    let domain = url.host_str().unwrap();

    let client = reqwest::Client::builder()
        .resolve(domain, addr)
        .timeout(Duration::from_secs(15))
        .build()?;

    let _r = barrier.wait().await;
    let mut stream = client.get(url.clone()).send().await?;
    while *stop_rx.borrow() != "stop" {
        while let Some(chunk) = stream.chunk().await? {
            count += chunk.len() as u128;
            let _r = counter_tx.send(count);
            if *stop_rx.borrow() == "stop" {
                break;
            }
        }
        stream = client.get(url.clone()).send().await?;
    }

    Ok(true)
}

pub async fn request_http_upload(
    url: Url,
    addr: SocketAddr,
    barrier: Arc<Barrier>,
    counter_tx: Sender<u128>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let mut count = 0;
    let mut data = BytesMut::new();
    data.put(
        "0123456789AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz-="
            .repeat(512)
            .as_bytes(),
    );

    let s = async_stream::stream! {
        loop {
            let chunk: Result<BytesMut, std::io::Error> = Ok(data.clone());
            count += 32768;
            let _r = counter_tx.send(count);
            yield chunk;
        }
    };

    let body = Body::wrap_stream(s);
    let domain = url.host_str().unwrap();
    let client = reqwest::Client::builder().resolve(domain, addr).build()?;

    let _r = barrier.wait().await;
    let _res = client
        .post(url.clone())
        .body(body)
        .timeout(Duration::from_secs(15))
        .send()
        .await?;

    Ok(true)
}
