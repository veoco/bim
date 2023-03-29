mod models;

pub use models::{IdMessage, MachineData, Message, ServerData, Task, TaskServer};

use bim_core::utils::SpeedTestResult;
use log::debug;

pub fn get_machine_id(name: &str, token: &str) -> Result<IdMessage, String> {
    let data = MachineData {
        name: name.to_string(),
        token: token.to_string(),
    };
    let r = minreq::post("https://bench.im/api/machines/")
        .with_timeout(5)
        .with_json(&data)
        .map_err(|_| "Invalid json")?
        .send()
        .map_err(|_| "Network error")?;

    debug!("Status code: {}", r.status_code);
    if r.status_code != 201 {
        return Err("Invalid name or token".to_string());
    }

    let m = r.json::<IdMessage>().map_err(|_| "Upgrade required")?;
    Ok(m)
}

pub fn get_tasks(machine_id: i32, token: &str) -> Result<Vec<Task>, String> {
    let url = format!("https://bench.im/api/tasks/?token={token}&machine_id={machine_id}&status=2");
    let r = minreq::get(&url)
        .with_timeout(5)
        .send()
        .map_err(|_| "Network error")?;

    debug!("Status code: {}", r.status_code);
    if r.status_code != 200 {
        return Err("Invalid name or token".to_string());
    }

    let tasks = r.json::<Vec<Task>>().map_err(|_| "Upgrade required")?;
    Ok(tasks)
}

pub fn finish_task(task_id: i32, token: &str, result: SpeedTestResult) -> Result<bool, String> {
    let url = format!("https://bench.im/api/tasks/{task_id}?token={token}");
    let r = minreq::post(url)
        .with_timeout(5)
        .with_json(&result)
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

pub fn get_ip(ipv6: bool) -> Result<String, String> {
    let url = if ipv6 {
        format!("http://ipv6.ip.sb")
    } else {
        format!("http://ipv4.ip.sb")
    };

    let r = minreq::get(url)
        .with_header("User-Agent", "curl/7.74.0")
        .with_header("Accept", "*/*")
        .with_timeout(5)
        .send()
        .map_err(|_| "Network error")?;

    let res = r.as_str().map_err(|_| "Upgrade required")?.to_string();
    if r.status_code == 200 {
        let ip = res.trim_end();
        Ok(ip.to_string())
    } else {
        Err(format!("{}", r.status_code))
    }
}

pub fn mask_ipv4(ipv4: &str) -> String {
    let parts: Vec<&str> = ipv4.split(" ").collect();
    format!("{}.*.*.{}", parts[0], parts[parts.len() - 1])
}

pub fn create_server(data: ServerData) -> Result<bool, String> {
    debug!("Data: {data:?}");

    let url = format!("https://bench.im/api/servers/");
    let r = minreq::post(url)
        .with_timeout(5)
        .with_json(&data)
        .map_err(|_| "Invalid json")?
        .send()
        .map_err(|_| "Network error")?;

    debug!("Status code: {}", r.status_code);
    let _ = r.json::<IdMessage>().map_err(|_| "Upgrade required")?;
    if r.status_code == 201 {
        Ok(true)
    } else {
        Err(format!("{} {:?}", r.status_code, r.as_str()))
    }
}
