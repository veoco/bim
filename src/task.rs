use std::collections::HashMap;
use std::error::Error;

use serde_json::Value;
use crate::speedtest::SpeedTest;


pub async fn register_machine(email: &str, token: &str) -> Result<String, Box<dyn Error>> {
    let machine_id = machine_uid::get()?;
    let mut map = HashMap::new();
    map.insert("email", email);
    map.insert("token", token);
    map.insert("machine_id", &machine_id);
    let url = "https://bench.im/api/machine/";
    let url = "http://127.0.0.1:5173/api/machine/";

    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .json(&map)
        .send()
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    let machine_id = res.get("pk").unwrap_or(&"".to_string()).clone();
    Ok(machine_id)
}

pub async fn get_tasks(
    machine_id: &str,
    email: &str,
    token: &str,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let url = format!(
        "https://bench.im/api/machine_tasks/?machine_id={}&email={}&token={}",
        machine_id, email, token
    );
    let url = format!(
        "http://127.0.0.1:5173/api/machine_tasks/?machine_id={}&email={}&token={}",
        machine_id, email, token
    );

    let client = reqwest::Client::new();
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


pub async fn send_result(task_id: &str, email: &str, token: &str, speedtest: &SpeedTest) -> Result<bool, Box<dyn Error>> {
    let result = speedtest.get_result();
    let upload = result.0.to_string();
    let download = result.1.to_string();
    let ping = result.2.to_string();
    let mut map = HashMap::new();
    map.insert("email", email);
    map.insert("token", token);
    map.insert("task_id", task_id);
    map.insert("upload", &upload);
    map.insert("download", &download);
    map.insert("ping", &ping);
    let url = "https://bench.im/api/result/";
    let url = "http://127.0.0.1:5173/api/result/";

    let client = reqwest::Client::new();
    let _res = client
        .post(url)
        .json(&map)
        .send()
        .await?;
    Ok(true)
}
