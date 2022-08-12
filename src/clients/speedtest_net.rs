use std::error::Error;
use std::io::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{
    watch::{self, Receiver},
    Barrier,
};
use tokio::time::{interval, sleep, timeout};

use crate::requests::{request_tcp_download, request_tcp_ping, request_tcp_upload};
use crate::utils::{format_size, BLUE, BOLD, ENDC, GREEN, RED};

pub struct SpeedtestNetClient {
    pub name: String,
    pub host: String,
    pub thread: u8,
    pub result: (u128, u128, u128),
}

impl SpeedtestNetClient {
    async fn ping(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut count = 5;
        let mut ping_min = 10_000_000;

        while count != 0 {
            let task = request_tcp_ping(&self.host);
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

        for _i in 0..self.thread {
            let host = self.host.clone();
            let b = barrier.clone();
            let mut r = stop_rx.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_tcp_download(&host, b, &mut r, c_tx).await });
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
                self.result.1 = (num - start) / (i - 2);
                self.show();
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
        let (stop_tx, stop_rx) = watch::channel("run");
        let mut counters: Vec<Receiver<u128>> = vec![];

        for _i in 0..self.thread {
            let host = self.host.clone();
            let b = barrier.clone();
            let mut r = stop_rx.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_tcp_upload(&host, b, &mut r, c_tx).await });
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
                self.result.0 = (num - start) / (i - 2);
                self.show();
            } else {
                if i == 2 {
                    start = num;
                }
                self.result.0 = num - last;
                self.show();
                last = num;
            }
        }
        stop_tx.send("stop")?;
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