use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{BufMut, BytesMut};
use reqwest::{header, Body, Url};
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

pub async fn request_http_download(
    url: Url,
    addr: SocketAddr,
    barrier: Arc<Barrier>,
    stop_rx: Receiver<&str>,
    counter_tx: Sender<u128>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let mut count = 0;

    let domain = url.host_str().unwrap();
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("bim 1"),
    );

    let client = reqwest::Client::builder()
        .resolve(domain, addr)
        .default_headers(headers)
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
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("bim 1"),
    );
    let client = reqwest::Client::builder()
        .resolve(domain, addr)
        .default_headers(headers)
        .build()?;

    let _r = barrier.wait().await;
    let _res = client
        .post(url.clone())
        .body(body)
        .timeout(Duration::from_secs(15))
        .send()
        .await?;

    Ok(true)
}
