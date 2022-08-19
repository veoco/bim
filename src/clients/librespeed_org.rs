use std::error::Error;
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

use crate::clients::{
    requests::{request_http_download, request_http_upload, request_tcp_ping},
    utils::Speedtest,
};

pub struct LibreSpeedOrgClient {
    pub name: String,
    pub download_url: String,
    pub upload_url: String,
    pub ipv6: bool,
    pub thread: u8,
    pub result: (u128, u128, u128),
}

impl Speedtest for LibreSpeedOrgClient {
    fn get_ping(&self) -> u128 {
        self.result.2
    }

    fn get_upload(&self) -> u128 {
        self.result.0
    }

    fn get_download(&self) -> u128 {
        self.result.1
    }

    fn get_name(&self) -> &str {
        &self.name
    }
}

impl LibreSpeedOrgClient {
    async fn resolve_ip(&self) -> Result<Option<SocketAddr>, Box<dyn Error>> {
        let resolver =
            TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())?;
        let url = Url::parse(&self.download_url)?;
        let host = url.host_str().unwrap();
        let port = url.port_or_known_default().unwrap();
        if self.ipv6 {
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

    async fn ping(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut count = 5;
        let mut ping_min = 10_000_000;

        let address = self.resolve_ip().await?;
        let addr = match address {
            Some(addr) => addr,
            None => return Ok(false),
        };

        while count != 0 {
            let task = request_tcp_ping(&addr);
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

        let address = self.resolve_ip().await?;
        let addr = match address {
            Some(addr) => addr,
            None => return Ok(false),
        };

        for _i in 0..self.thread {
            let url = url.clone();
            let a = addr.clone();
            let b = barrier.clone();
            let mut r = stop_rx.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_http_download(url, a, b, &mut r, c_tx).await });
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

        let address = self.resolve_ip().await?;
        let addr = match address {
            Some(addr) => addr,
            None => return Ok(false),
        };

        for _i in 0..self.thread {
            let url = url.clone();
            let a = addr.clone();
            let b = barrier.clone();
            let (c_tx, c_rx) = watch::channel(0);
            counters.push(c_rx);
            tokio::spawn(async move { request_http_upload(url, a, b, c_tx).await });
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
