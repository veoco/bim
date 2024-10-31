mod models;

use std::sync::Arc;

use regex::Regex;
use tokio::process::Command;
use tokio::sync::Semaphore;

pub use models::{Machine, MachineData, Message, PingData, Target};

use log::{debug, info};

pub fn get_machine_id(name: &str, token: &str, server_url: &str) -> Result<Machine, String> {
    let data = MachineData {
        name: name.to_string(),
    };
    let url = format!("{server_url}/api/client/machines/");
    debug!("Url: {url}");

    let r = minreq::post(url)
        .with_header("Authorization", format!("Bearer {token}"))
        .with_timeout(5)
        .with_json(&data)
        .map_err(|_| "Invalid json")?
        .send()
        .map_err(|_| "Network error")?;

    debug!("Status code: {}", r.status_code);
    if r.status_code != 201 {
        debug!("Content: {:?}", r.as_str());
        return Err("Invalid name or token".to_string());
    }

    let m = r.json::<Machine>().map_err(|_| "Upgrade required")?;
    Ok(m)
}

pub fn get_targets(token: &str, server_url: &str) -> Result<Vec<Target>, String> {
    let url = format!("{server_url}/api/client/targets/");
    let r = minreq::get(&url)
        .with_header("Authorization", format!("Bearer {token}"))
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

pub async fn ping(target: String, ipv6: bool, s: Arc<Semaphore>) -> Option<PingData> {
    let _permit = match s.acquire().await {
        Ok(p) => p,
        _ => {
            debug!("Acquire semaphore failed");

            return None;
        }
    };

    let (net_arg, count_arg) = if cfg!(target_os = "windows") {
        if ipv6 {
            ("-6", "-n")
        } else {
            ("-4", "-n")
        }
    } else {
        if ipv6 {
            ("-6", "-c")
        } else {
            ("-4", "-c")
        }
    };

    let output = Command::new("ping")
        .arg(count_arg)
        .arg("20")
        .arg(net_arg)
        .arg(target)
        .output()
        .await
        .expect("Failed to execute ping command");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let mut ping_times = Vec::new();
    let mut ping_success = 0;

    let time_regex = Regex::new(r"=([\d.]+) ?ms").unwrap();
    let mut line_count = 0;
    for line in stdout.lines() {
        if let Some(caps) = time_regex.captures(line) {
            if let Ok(time) = caps[1].parse::<f64>() {
                ping_times.push(time);
                ping_success += 1;
            }
        }

        if line_count > 0 && line.is_empty() {
            break;
        }

        line_count += 1;
    }

    if ping_success == 0 {
        return None;
    }

    let ping_min = ping_times
        .iter()
        .copied()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    let ping_jitter = if ping_times.len() > 1 {
        let mean = ping_times.iter().copied().sum::<f64>() / ping_times.len() as f64;
        let variance = ping_times.iter().map(|x| (*x - mean).powi(2)).sum::<f64>()
            / (ping_times.len() as f64 - 1.0);
        variance.sqrt()
    } else {
        0.0
    };

    let ping_failed = 20 - ping_success;

    Some(PingData {
        ipv6,
        min: ping_min,
        jitter: ping_jitter,
        failed: ping_failed,
    })
}

pub fn add_target_data(
    machine_id: i32,
    target_id: i32,
    token: &str,
    server_url: &str,
    data: PingData,
) {
    let url = format!("{server_url}/api/client/machines/{machine_id}/targets/{target_id}");

    let mut retry = 3;

    while retry > 0 {
        let request = match minreq::post(&url)
            .with_header("Authorization", format!("Bearer {token}"))
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
        match response.json::<Message>() {
            Ok(_) => {
                debug!("Add target {target_id} data success");
            }
            Err(e) => {
                debug!("Add target data failed: {e}");
                info!("Upgrade required");
            }
        };
        return;
    }
}
