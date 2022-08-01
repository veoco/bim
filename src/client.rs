use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::watch::Receiver;
use tokio::sync::{watch, Mutex};
use tokio::time::{sleep, timeout, interval};

use crate::utils::{format_size, RED, BLUE, GREEN, BOLD, ENDC};


pub struct SpeedtestClient {
    pub name: String,
    pub host: String,
    pub thread: u8,
    pub result: (u64, u64, u128),
}

impl SpeedtestClient {
    async fn ping(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut count = 10;
        let mut ping_min = 10000;
        while count != 0 {
            let task = request_ping(&self.host);
            let ping_ms = timeout(Duration::from_millis(1000), task)
                .await
                .unwrap_or(Ok(10000))
                .unwrap_or(10000);
            if ping_ms < ping_min {
                ping_min = ping_ms;
            }
            self.result.2 = if ping_min != 10000 {
                ping_min
            } else {
                return Ok(false);
            };
            self.show(false);
            sleep(Duration::from_millis(300)).await;
            count -= 1;
        }
        self.result.2 = if ping_min != 10000 { ping_min } else { 0 };
        self.show(false);
        Ok(true)
    }

    async fn download(&mut self) -> Result<bool, Box<dyn Error>> {
        let (tx, rx) = watch::channel("ready");
        let counter: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
        let mut last: u64 = 0;

        for _i in 0..self.thread {
            let host = self.host.clone();
            let mut r = rx.clone();
            let c = Arc::clone(&counter);
            tokio::spawn(async move { request_download(&host, &mut r, c).await });
        }

        let mut time_interval = interval(Duration::from_millis(500));
        tx.send("downlaod")?;
        time_interval.tick().await;

        for _i in 0..30 {
            time_interval.tick().await;
            let num = {*(counter.lock().await)};
            self.result.1 = (num - last) << 1;
            last = num;
            self.show(false);
        }
        tx.send("stop")?;
        sleep(Duration::from_millis(200)).await;

        Ok(true)
    }

    async fn upload(&mut self) -> Result<bool, Box<dyn Error>> {
        let (tx, rx) = watch::channel("ready");
        let counter: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
        let mut last: u64 = 0;

        for _i in 0..self.thread {
            let host = self.host.clone();
            let mut r = rx.clone();
            let c = Arc::clone(&counter);
            tokio::spawn(async move { request_upload(&host, &mut r, c).await });
        }

        let mut time_interval = interval(Duration::from_millis(500));
        tx.send("upload")?;
        time_interval.tick().await;

        for _i in 0..30 {
            time_interval.tick().await;
            let num = {*(counter.lock().await)};
            self.result.0 = (num - last) << 1;
            last = num;
            self.show(false);
        }
        tx.send("stop")?;
        sleep(Duration::from_millis(200)).await;

        Ok(true)
    }

    fn show(&self, last: bool) {
        let upload = if self.result.0 != 0 {
            format_size(&self.result.0)
        } else {
            if last {
                "Failed".to_string()
            } else {
                "Waiting".to_string()
            }
        };
        let download = if self.result.1 != 0 {
            format_size(&self.result.1)
        } else {
            if last {
                "Failed".to_string()
            } else {
                "Waiting".to_string()
            }
        };
        let ping = if self.result.2 != 0 {
            format!("{}ms", self.result.2)
        } else {
            if last {
                "Failed".to_string()
            } else {
                "Waiting".to_string()
            }
        };

        print!(
            "\r{BOLD}{}{BLUE}{:>12}{ENDC}{RED}{:>12}{ENDC}{GREEN}{:>10}{ENDC}{ENDC}",
            self.name, upload, download, ping
        );
    }

    pub async fn run(&mut self) -> bool {
        self.show(false);
        let ping = self.ping().await.unwrap_or(false);
        if !ping {
            self.result.2 = 0;
            self.show(true);
            sleep(Duration::from_secs(3)).await;
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
            self.show(true);
            println!("");
        }
        true
    }
}

async fn request_ping(host: &str) -> Result<u128, Box<dyn Error + Send + Sync>> {
    let command = "PING 0\n";

    let mut stream = TcpStream::connect(&host).await?;
    stream.set_nodelay(true).unwrap();

    let now = Instant::now();
    stream.write_all(command.as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut buffer = String::new();
    reader.read_line(&mut buffer).await?;

    Ok(now.elapsed().as_millis())
}

async fn request_download(
    host: &str,
    rx: &mut Receiver<&str>,
    counter: Arc<Mutex<u64>>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let data_size: u64 = 15 * 1024 * 1024 * 1024;
    let command = format!("DOWNLOAD {}\n", data_size);

    let mut stream = TcpStream::connect(&host).await?;
    stream.set_nodelay(true).unwrap();
    let _r = rx.changed().await.is_ok();

    stream.write_all(command.as_bytes()).await?;

    let mut reader = BufReader::new(stream);

    while !rx.has_changed().unwrap_or(false) {
        reader.read_exact(&mut [0; 16384]).await?;
        let mut num = counter.lock().await;
        *num += 1;
    }
    Ok(true)
}

async fn request_upload(
    host: &str,
    rx: &mut Receiver<&str>,
    counter: Arc<Mutex<u64>>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let data_size: u64 = 15 * 1024 * 1024 * 1024;
    let data = "23456789ABCDEFGHIJKLMNOPQRSTUVWX".repeat(512);
    let data = data.as_bytes();

    let command = format!("UPLOAD {} 0\n", data_size);

    let mut stream = TcpStream::connect(&host).await?;
    let _r = rx.changed().await.is_ok();
    stream.write(command.as_bytes()).await?;

    while !rx.has_changed().unwrap_or(false) {
        stream.write_all(data).await?;
        let mut num = counter.lock().await;
        *num += 1;
    }

    Ok(true)
}
