mod models;

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::thread;
use std::time::{Duration, Instant};

use url::Url;

pub use models::{Machine, MachineData, Target, TcpingData, Message};

use log::debug;

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

pub fn get_targets(machine_id: i32) -> Result<Vec<Target>, String> {
    let url = format!("https://bench.im/api/machines/{machine_id}/targets/latest");
    let r = minreq::post(&url)
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

pub fn add_target_data(
    machine_id:i32,
    target_id: i32,
    token: &str,
    data: TcpingData,
) -> Result<bool, String> {
    let url = format!("/machines/{machine_id}/targets/{target_id}/");
    let r = minreq::post(url)
        .with_header("X-API-Key", token)
        .with_timeout(5)
        .with_json(&data)
        .map_err(|_| "Invalid json")?
        .send()
        .map_err(|_| "Network error")?;

    debug!("Status code: {}", r.status_code);
    let res = r.json::<Message>().map_err(|_| "Upgrade required")?;
    if r.status_code == 200 {
        Ok(true)
    } else {
        Err(res.msg)
    }
}

fn request_tcp_ping(address: &SocketAddr) -> u128 {
    let now = Instant::now();
    let r = TcpStream::connect_timeout(&address, Duration::from_micros(1_000_000));
    let used = now.elapsed().as_micros();
    match r {
        Ok(_) => used,
        Err(_e) => {
            #[cfg(debug_assertions)]
            debug!("Ping {_e}");

            0
        }
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

pub fn test_tcp_pings(url: String, ipv6: bool) -> Option<TcpingData> {
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
        let ping = request_tcp_ping(&address);
        if ping > 0 {
            if ping < ping_min {
                ping_min = ping
            }
            pings[count] = ping;
        }
        thread::sleep(Duration::from_millis(1000));
        count += 1;
    }

    let mut failed = 0;
    let mut jitter_all = 0;
    for p in pings {
        if p > 0 {
            jitter_all += p - ping_min;
        } else {
            failed += 1;
        }
    }

    let ping_min = ping_min as f64 / 1_000.0;
    let ping_jitter = jitter_all as f64 / (20 - failed) as f64 / 1_000.0;

    #[cfg(debug_assertions)]
    debug!("Ping {ping_min} ms, Jitter {ping_jitter} ms, Failed {failed}/20");

    Some(TcpingData {
        ping_min,
        ping_jitter,
        failed,
    })
}
