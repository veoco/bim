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
    pub url: String,
    pub ipv6: bool,
}

#[derive(Serialize, Deserialize)]
pub struct TcpingData {
    pub ping_min: f64,
    pub ping_jitter: f64,
    pub ping_failed: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub msg: String,
}
