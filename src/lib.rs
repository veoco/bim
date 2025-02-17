mod models;

use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use tokio::process::Command;
use tokio::sync::Semaphore;

pub use models::{Machine, MachineData, Message, PingData, Target};

use log::{debug, info};

pub struct BimClient {
    pub name: String,
    pub token: String,
    pub server_url: String,
    pub machine_id: i32,
    pub client: reqwest::Client,
}

impl BimClient {
    pub async fn new(name: String, token: String, server_url: String) -> Result<Self, String> {
        let client = reqwest::Client::new();

        let data = MachineData {
            name: name.to_string(),
        };
        let url = format!("{server_url}/api/client/machines/");
        debug!("Url: {url}");

        let r = client
            .post(url)
            .bearer_auth(&token)
            .timeout(Duration::from_secs(5))
            .json(&data)
            .send()
            .await
            .map_err(|_| "Network error")?;

        debug!("Status code: {}", r.status());
        if r.status() != 201 {
            debug!("Content: {:?}", r.text().await);
            return Err("Invalid name or token".to_string());
        }

        let m = r.json::<Machine>().await.map_err(|_| "Upgrade required")?;
        let machine_id = m.id;
        info!("Machine id: {machine_id}");

        Ok(Self {
            name,
            token,
            server_url,
            machine_id,
            client,
        })
    }

    pub async fn get_targets(&self) -> Result<Vec<Target>, String> {
        let url = format!("{}/api/client/targets/", self.server_url);
        let r = self
            .client
            .get(&url)
            .bearer_auth(&self.token)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|_| "Network error")?;

        debug!("Status code: {}", r.status());
        if r.status() != 200 {
            return Err("Invalid name or token".to_string());
        }

        let targets = r
            .json::<Vec<Target>>()
            .await
            .map_err(|_| "Upgrade required")?;
        Ok(targets)
    }

    pub async fn post_target_data(&self, target_id: i32, data: PingData) {
        let url = format!(
            "{}/api/client/machines/{}/targets/{}",
            self.server_url, self.machine_id, target_id
        );

        let r = match self
            .client
            .post(&url)
            .bearer_auth(&self.token)
            .timeout(Duration::from_secs(5))
            .json(&data)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                debug!("Add target data failed: {e}");
                return;
            }
        };

        debug!("Status code: {}", r.status());
        match r.json::<Message>().await {
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

pub async fn ping(
    target: String,
    ipv6: bool,
    s: Arc<Semaphore>,
    target_id: i32,
    cc: Arc<BimClient>,
) {
    let permit = match s.acquire().await {
        Ok(p) => p,
        _ => {
            debug!("Acquire semaphore failed");

            return;
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

    drop(permit);

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
        return;
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

    let data = PingData {
        ipv6,
        min: ping_min,
        jitter: ping_jitter,
        failed: ping_failed,
    };

    cc.post_target_data(target_id, data).await
}
