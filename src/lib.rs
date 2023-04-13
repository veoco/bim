mod models;

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time;
use url::Url;

pub use models::{Machine, MachineData, Message, Target, TcpingData};

use log::{debug, info};

pub fn get_machine_id(name: &str, token: &str) -> Result<Machine, String> {
    let data = MachineData {
        name: name.to_string(),
    };
    let r = minreq::post("https://bench.im/api/machines/")
        .with_header("X-API-Key", token)
        .with_timeout(5)
        .with_json(&data)
        .map_err(|_| "Invalid json")?
        .send()
        .map_err(|_| "Network error")?;

    debug!("Status code: {}", r.status_code);
    if r.status_code != 201 {
        return Err("Invalid name or token".to_string());
    }

    let m = r.json::<Machine>().map_err(|_| "Upgrade required")?;
    Ok(m)
}

pub fn get_targets(token: &str) -> Result<Vec<Target>, String> {
    let url = format!("https://bench.im/api/targets/worker");
    let r = minreq::post(&url)
        .with_header("X-API-Key", token)
        .with_timeout(5)
        .send()
        .map_err(|_| "Network error")?;

    debug!("Status code: {}", r.status_code);
    if r.status_code != 200 {
        return Err("Invalid name or token".to_string());
    }

    let targets = r.json::<Vec<Target>>().map_err(|_| "Upgrade required")?;
    Ok(targets)
}

pub fn add_target_data(machine_id: i32, target_id: i32, token: &str, data: TcpingData) {
    let url = format!("https://bench.im/api/machines/{machine_id}/targets/{target_id}/");

    let mut retry = 3;

    while retry > 0 {
        let request = match minreq::post(&url)
            .with_header("X-API-Key", token)
            .with_timeout(5)
            .with_json(&data)
        {
            Ok(r) => r,
            Err(e) => {
                debug!("Add target data failed: {e}");
                return;
            }
        };

        let response = match request.send() {
            Ok(r) => r,
            Err(e) => {
                debug!("Add target data failed: {e}");
                retry -= 1;
                continue;
            }
        };

        debug!("Status code: {}", response.status_code);
        match response.json::<Message>(){
            Ok(_)=> {
                debug!("Add target {target_id} data success");
            },
            Err(e)=> {
                debug!("Add target data failed: {e}");
                info!("Upgrade required");
            }
        };
        return
    }
}

fn get_address(url: &Url, ipv6: bool) -> Option<SocketAddr> {
    let host = match url.host_str() {
        Some(h) => h,
        None => return None,
    };
    let port = match url.port_or_known_default() {
        Some(p) => p,
        None => return None,
    };

    let host_port = format!("{host}:{port}");
    let addresses = match host_port.to_socket_addrs() {
        Ok(addrs) => addrs,
        Err(_) => return None,
    };

    let mut address = None;
    for addr in addresses {
        if (addr.is_ipv6() && ipv6) || (addr.is_ipv4() && !ipv6) {
            address = Some(addr);
        }
    }

    address
}

async fn request_tcp_ping(address: &SocketAddr, s: Arc<Semaphore>) -> u128 {
    let _permit = match s.acquire().await {
        Ok(p) => p,
        _ => {
            debug!("Acquire semaphore failed");

            return 0;
        }
    };

    let now = Instant::now();

    let r = time::timeout(
        Duration::from_micros(1_000_000),
        TcpStream::connect(&address),
    )
    .await;

    let used = now.elapsed().as_micros();

    match r {
        Ok(Ok(_)) => used,
        Ok(Err(_e)) => {
            debug!("Ping {_e}");

            0
        }
        Err(_) => {
            debug!("Ping timeout");

            0
        }
    }
}

pub async fn test_tcp_pings(url: String, ipv6: bool, s: Arc<Semaphore>) -> Option<TcpingData> {
    let mut count = 0;
    let mut pings = [0u128; 20];
    let mut ping_min = 10000000;

    let url = match Url::parse(&url) {
        Ok(u) => u,
        Err(_) => return None,
    };

    let address = match get_address(&url, ipv6) {
        Some(a) => a,
        None => return None,
    };

    while count < 20 {
        let ping = request_tcp_ping(&address, s.clone()).await;
        if ping > 0 {
            if ping < ping_min {
                ping_min = ping
            }
            pings[count] = ping;
        }
        count += 1;
    }

    let mut ping_failed = 0;
    let mut jitter_all = 0;
    for p in pings {
        if p > 0 {
            jitter_all += p - ping_min;
        } else {
            ping_failed += 1;
        }
    }

    let ping_min = ping_min as f64 / 1_000.0;
    let ping_jitter = jitter_all as f64 / (20 - ping_failed) as f64 / 1_000.0;

    debug!("Ping {ping_min} ms, Jitter {ping_jitter} ms, Failed {ping_failed}/20");

    Some(TcpingData {
        ping_min,
        ping_jitter,
        ping_failed,
    })
}
