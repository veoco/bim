use std::collections::HashMap;
use std::env;
use std::error::Error;

use reqwest::Client;
use log::{debug, info};
use serde_json::Value;

use crate::speedtest::SpeedTest;

pub async fn register_machine(client: &Client) -> Result<String, Box<dyn Error>> {
    let machine_id = machine_uid::get()?;
    debug!("Machine ID: {}", machine_id);
    let mut map = HashMap::new();
    map.insert("machine_id", &machine_id);

    let url = format!(
        "{}api/machine/",
        env::var("BENCH_URL").unwrap_or(String::from("https://bench.im/"))
    );

    let res = client
        .post(url)
        .json(&map)
        .send()
        .await?;
    if res.status() != 200 {
        debug!("Response: {}", res.text().await?);
        Ok(String::from(""))
    } else{
        let json = res.json::<HashMap<String, String>>().await?;
        let machine_id = json.get("pk").unwrap_or(&"".to_string()).clone();
        Ok(machine_id)
    }
}

pub async fn get_tasks(machine_id: &str, client: &Client) -> Result<Vec<Value>, Box<dyn Error>> {
    let url = format!(
        "{}api/machine_tasks/?machine_id={}",
        env::var("BENCH_URL").unwrap_or(String::from("https://bench.im/")),
        machine_id
    );

    let tasks = client
        .get(url)
        .send()
        .await?
        .json::<Value>()
        .await?
        .get("results")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    Ok(tasks)
}

pub async fn send_result(
    task_id: &str,
    client: &Client,
    speedtest: &SpeedTest,
) -> Result<bool, Box<dyn Error>> {
    let result = speedtest.get_result();
    let upload = result.0.to_string();
    let download = result.1.to_string();
    let ping = result.2.to_string();
    let mut map = HashMap::new();
    map.insert("task_id", task_id);
    map.insert("upload", &upload);
    map.insert("download", &download);
    map.insert("ping", &ping);
    let url = format!(
        "{}api/result/",
        env::var("BENCH_URL").unwrap_or(String::from("https://bench.im/"))
    );

    info!("Task {} upload {} download {} ping {}", task_id, upload, download, ping);

    let _res = client.post(url).json(&map).send().await?;
    Ok(true)
}
