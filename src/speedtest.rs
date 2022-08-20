use std::error::Error;
use std::io::prelude::*;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;
use tokio::sync::{
    watch::{self, Receiver},
    Barrier,
};
use tokio::time::{interval, sleep, timeout};
use trust_dns_resolver::config::*;
use trust_dns_resolver::TokioAsyncResolver;

use crate::requests::{request_http_download, request_http_upload, request_tcp_ping};
use crate::utils::{BLUE, BOLD, ENDC, GREEN, RED};

pub struct SpeedTest {
    pub provider: String,
    pub name: String,
    pub download_url: String,
    pub upload_url: String,
    pub state: String,

    pub ipv6: bool,
    pub thread: u8,

    address: SocketAddr,

    upload: [u128; 14],
    download: [u128; 14],
    ping: [u128; 20],
    index: [u8; 3],
}

impl SpeedTest {
    pub async fn build(
        provider: String,
        name: String,
        download_url: String,
        upload_url: String,
        ipv6: bool,
        thread: u8,
    ) -> Option<SpeedTest> {
        let address = SpeedTest::resolve_ip(&download_url, ipv6)
            .await
            .unwrap_or(None)?;

        Some(SpeedTest {
            provider,
            name,
            download_url,
            upload_url,
            state: String::from("waiting"),
            ipv6,
            thread,
            address,
            upload: [0; 14],
            download: [0; 14],
            ping: [0; 20],
            index: [0; 3],
        })
    }

    pub async fn resolve_ip(url: &str, ipv6: bool) -> Result<Option<SocketAddr>, Box<dyn Error>> {
        let resolver =
            TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())?;
        let url = Url::parse(url)?;
        let host = url.host_str().unwrap();
        let port = url.port_or_known_default().unwrap();
        if ipv6 {
            let response = resolver.ipv6_lookup(host).await?;
            let address = response.into_iter().next();
            if let Some(addr) = address {
                return Ok(Some(SocketAddr::new(IpAddr::V6(addr), port)));
            }

            return Ok(None);
        } else {
            let response = resolver.ipv4_lookup(host).await?;
            let address = response.into_iter().next();
            if let Some(addr) = address {
                return Ok(Some(SocketAddr::new(IpAddr::V4(addr), port)));
            }

            return Ok(None);
        }
    }

    fn get_speed(&self, array: [u128; 14], i: usize) -> f64 {
        let pos = self.index[i] as usize;
        if pos == 0 {
            return 0.0;
        }
        if pos <= 3 {
            return array[pos - 1] as f64 / pos as f64;
        } else {
            let base = array[2];
            return (array[pos - 1] - base) as f64 / (pos - 3) as f64;
        }
    }

    fn get_upload(&self) -> f64 {
        self.get_speed(self.upload, 0)
    }

    fn set_upload(&mut self, upload: u128) {
        self.upload[self.index[0] as usize] = upload;
        self.index[0] = self.index[0] + 1
    }

    fn get_download(&self) -> f64 {
        self.get_speed(self.download, 1)
    }

    fn set_download(&mut self, download: u128) {
        self.download[self.index[1] as usize] = download;
        self.index[1] = self.index[1] + 1
    }

    fn get_ping(&self) -> f64 {
        let pos = self.index[2] as usize;
        let mut sum = 0;
        let mut ping_min = 1_000_000;
        let mut ping_max = 0;
        if pos <= 2 {
            for i in (0..pos).rev() {
                sum += self.ping[i];
            }
            return sum as f64 / pos as f64;
        } else {
            for i in (0..pos).rev() {
                let ping = self.ping[i];
                sum += ping;
                if ping < ping_min {
                    ping_min = ping;
                    continue;
                }
                if ping > ping_max {
                    ping_max = ping;
                }
            }
            return (sum - ping_max - ping_min) as f64 / (pos - 2) as f64;
        }
    }

    fn set_ping(&mut self, ping: u128) {
        self.ping[self.index[2] as usize] = ping;
        self.index[2] = self.index[2] + 1
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_result(&self) -> (f64, f64, f64) {
        let upload = self.get_upload() / 125_000.0;
        let download = self.get_download() / 125_000.0;
        let ping = self.get_ping() / 1000.0;
        (upload, download, ping)
    }

    fn write_stdout(&self, name: &str, upload: f64, download: f64, ping: f64) {
        let upload = if self.index[0] != 0 {
            format!("{:.1} Mbps", upload)
        } else {
            "-".to_string()
        };
        let download = if self.index[1] != 0 {
            format!("{:.1} Mbps", download)
        } else {
            "-".to_string()
        };
        let ping = if self.index[2] != 0 {
            format!("{:.1} ms", ping)
        } else {
            "-".to_string()
        };
        let line = format!(
            "\r{BOLD}{}{BLUE}{:>12}{ENDC}{RED}{:>12}{ENDC}{GREEN}{:>10}{ENDC}{ENDC}",
            name, upload, download, ping
        );
        let mut stdout = std::io::stdout();
        let _r = stdout.write_all(line.as_bytes());
        let _r = stdout.flush();
    }

    fn show(&self) {
        let (upload, download, ping) = self.get_result();
        let name = self.get_name();

        self.write_stdout(name, upload, download, ping)
    }

    async fn ping(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut count = 5;

        while count != 0 {
            let task = request_tcp_ping(&self.address);
            let ping = timeout(Duration::from_micros(1_000_000), task)
                .await
                .unwrap_or(Ok(1_000_000))
                .unwrap_or(1_000_000);
            self.set_ping(ping);
            self.show();
            sleep(Duration::from_millis(500)).await;
            count -= 1;
        }
        Ok(true)
    }

    async fn download(&mut self) -> Result<bool, Box<dyn Error>> {
        let barrier = Arc::new(Barrier::new((self.thread + 1) as usize));
        let (stop_tx, stop_rx) = watch::channel("run");
        let mut counters: Vec<Receiver<u128>> = vec![];

        let mut url = Url::parse(&self.download_url)?;
        if self.provider == "LibreSpeed" {
            url.set_query(Some("ckSize=1024"));
        }

        for _i in 0..self.thread {
            let url = url.clone();
            let a = self.address.clone();
            let b = barrier.clone();
            let r = stop_rx.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_http_download(url, a, b, r, c_tx).await });
        }

        let mut time_interval = interval(Duration::from_millis(1000));
        let _r = barrier.wait().await;
        time_interval.tick().await;

        for _i in 1..15 {
            time_interval.tick().await;
            let num = {
                let mut count = 0;
                for counter in counters.iter() {
                    count += *counter.borrow();
                }
                count
            };
            self.set_download(num);
            self.show();
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
            let a = self.address.clone();
            let b = barrier.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_http_upload(url, a, b, c_tx).await });
        }

        let mut time_interval = interval(Duration::from_millis(1000));
        let _r = barrier.wait().await;
        time_interval.tick().await;

        for _i in 1..15 {
            time_interval.tick().await;
            let num = {
                let mut count = 0;
                for counter in counters.iter() {
                    count += *counter.borrow();
                }
                count
            };
            self.set_upload(num);
            self.show();
        }
        sleep(Duration::from_secs(1)).await;

        Ok(true)
    }

    pub async fn run(&mut self) -> bool {
        let ping = self.ping().await.unwrap_or(false);
        if !ping {
            sleep(Duration::from_secs(1)).await;
            return false;
        } else {
            let _download = self.download().await.unwrap_or(false);
            let _upload = self.upload().await.unwrap_or(false);
            println!("");
        }
        true
    }
}
