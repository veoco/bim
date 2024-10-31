use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MachineData {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Machine {
    pub id: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Target {
    pub id: i32,
    pub name: String,
    pub domain: Option<String>,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PingData {
    pub ipv6: bool,
    pub min: f64,
    pub jitter: f64,
    pub failed: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub msg: String,
}
