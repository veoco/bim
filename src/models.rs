use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MachineData {
    pub name: String,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerData {
    pub name: String,
    pub token: String,
    pub download_url: String,
    pub upload_url: String,
    pub ipv6: bool,
    pub multi: bool,
}

#[derive(Serialize, Deserialize)]
pub struct TaskServer {
    pub name: String,
    pub download_url: String,
    pub upload_url: String,
    pub ipv6: bool,
    pub multi: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Task {
    pub id: i32,
    pub server: TaskServer,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub msg: String,
}

#[derive(Serialize, Deserialize)]
pub struct IdMessage {
    pub id: i32,
}
