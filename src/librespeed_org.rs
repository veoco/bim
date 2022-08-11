use std::error::Error;
use std::io::prelude::*;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{BufMut, BytesMut};
use reqwest::{Body, Url};
use tokio::net::TcpStream;
use tokio::sync::{
    watch::{self, Receiver, Sender},
    Barrier,
};
use tokio::time::{interval, sleep, timeout};

use crate::utils::{format_size, BLUE, BOLD, ENDC, GREEN, RED};

pub struct LibreSpeedOrgClient {
    pub name: String,
    pub download_url: String,
    pub upload_url: String,
    pub thread: u8,
    pub result: (u128, u128, u128),
}

impl LibreSpeedOrgClient {
    async fn ping(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut count = 5;
        let mut ping_min = 10_000_000;

        let url = Url::parse(&self.download_url)?;
        let addr = url.socket_addrs(|| None)?[0];

        while count != 0 {
            let task = request_ping(addr);
            let ping_ms = timeout(Duration::from_micros(10_000_000), task)
                .await
                .unwrap_or(Ok(10_000_000))
                .unwrap_or(10_000_000);
            if ping_ms < ping_min {
                ping_min = ping_ms;
            }
            self.result.2 = if ping_min != 10_000_000 {
                ping_min
            } else {
                return Ok(false);
            };
            self.show();
            sleep(Duration::from_millis(500)).await;
            count -= 1;
        }
        self.result.2 = if ping_min != 10000 { ping_min } else { 0 };
        self.show();
        Ok(true)
    }

    async fn download(&mut self) -> Result<bool, Box<dyn Error>> {
        let barrier = Arc::new(Barrier::new((self.thread + 1) as usize));
        let (stop_tx, stop_rx) = watch::channel("run");
        let mut counters: Vec<Receiver<u128>> = vec![];

        let mut url = Url::parse(&self.download_url)?;
        url.set_query(Some("ckSize=1024"));

        for _i in 0..self.thread {
            let url = url.clone();
            let b = barrier.clone();
            let mut r = stop_rx.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_download(url, b, &mut r, c_tx).await });
        }

        let mut last = 0;
        let mut start = 0;
        let mut time_interval = interval(Duration::from_millis(1000));
        let _r = barrier.wait().await;
        time_interval.tick().await;

        for i in 0..15 {
            time_interval.tick().await;
            let num = {
                let mut count = 0;
                for counter in counters.iter() {
                    count += *counter.borrow();
                }
                count
            };
            if i > 2 {
                let increment = num - start;
                if increment > 0 {
                    self.result.1 = increment / (i - 2);
                    self.show();
                }
            } else {
                if i == 2 {
                    start = num;
                }
                self.result.1 = num - last;
                self.show();
                last = num;
            }
        }
        stop_tx.send("stop")?;
        sleep(Duration::from_secs(1)).await;

        Ok(true)
    }

    async fn upload(&mut self) -> Result<bool, Box<dyn Error>> {
        let barrier = Arc::new(Barrier::new((self.thread + 1) as usize));
        let mut counters: Vec<Receiver<u128>> = vec![];

        let url = Url::parse(&self.upload_url)?;

        for _i in 0..self.thread {
            let url = url.clone();
            let b = barrier.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_upload(url, b, c_tx).await });
        }

        let mut last = 0;
        let mut start = 0;
        let mut time_interval = interval(Duration::from_millis(1000));
        let _r = barrier.wait().await;
        time_interval.tick().await;

        for i in 0..15 {
            time_interval.tick().await;
            let num = {
                let mut count = 0;
                for counter in counters.iter() {
                    count += *counter.borrow();
                }
                count
            };
            if i > 2 {
                let increment = num - start;
                if increment > 0 {
                    self.result.0 = increment / (i - 2);
                    self.show();
                }
            } else {
                if i == 2 {
                    start = num;
                }
                self.result.0 = num - last;
                self.show();
                last = num;
            }
        }
        sleep(Duration::from_secs(1)).await;

        Ok(true)
    }

    fn show(&self) {
        let upload = if self.result.0 != 0 {
            format_size(&self.result.0)
        } else {
            "-".to_string()
        };
        let download = if self.result.1 != 0 {
            format_size(&self.result.1)
        } else {
            "-".to_string()
        };
        let ping = if self.result.2 != 0 {
            let ping = if self.result.2 < 1000 {
                format!("<1 ms")
            } else {
                format!("{:.1} ms", self.result.2 as f64 / 1000.0)
            };
            ping
        } else {
            "-".to_string()
        };

        let line = format!(
            "\r{BOLD}{}{BLUE}{:>12}{ENDC}{RED}{:>12}{ENDC}{GREEN}{:>10}{ENDC}{ENDC}",
            self.name, upload, download, ping
        );
        let mut stdout = std::io::stdout();
        let _r = stdout.write_all(line.as_bytes());
        let _r = stdout.flush();
    }

    pub async fn run(&mut self) -> bool {
        self.show();
        let ping = self.ping().await.unwrap_or(false);
        if !ping {
            self.result.2 = 0;
            self.show();
            sleep(Duration::from_secs(1)).await;
            return false;
        } else {
            let download = self.download().await.unwrap_or(false);
            if !download {
                self.result.1 = 0;
            } else {
                let upload = self.upload().await.unwrap_or(false);
                if !upload {
                    self.result.0 = 0;
                }
            }
            self.show();
            println!("");
        }
        true
    }
}

async fn request_ping(addr: SocketAddr) -> Result<u128, Box<dyn Error + Send + Sync>> {
    let now = Instant::now();
    let _stream = TcpStream::connect(addr).await?;
    let used = now.elapsed().as_micros();
    Ok(used)
}

async fn request_download(
    url: Url,
    barrier: Arc<Barrier>,
    stop_rx: &mut Receiver<&str>,
    counter_tx: Sender<u128>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let mut count = 0;

    let client = reqwest::Client::builder()
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

async fn request_upload(
    url: Url,
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
    let client = reqwest::Client::builder().build()?;

    let _r = barrier.wait().await;
    let _res = client
        .post(url.clone())
        .body(body)
        .timeout(Duration::from_secs(15))
        .send()
        .await?;

    Ok(true)
}
