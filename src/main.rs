use std::error::Error;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::prelude::*;
use clap::Parser;
use serde_json::Value;
use tokio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::watch::Receiver;
use tokio::sync::{watch, Mutex};
use tokio::time::{sleep, timeout};

const RED: &str = "\x1b[31;1m";
const GREEN: &str = "\x1b[32;1m";
const BLUE: &str = "\x1b[94;1m";
const BOLD: &str = "\x1b[1m";
const ENDC: &str = "\x1b[0m";

const WIDTH: [(u32, u8); 38] = [
    (126, 1),
    (159, 0),
    (687, 1),
    (710, 0),
    (711, 1),
    (727, 0),
    (733, 1),
    (879, 0),
    (1154, 1),
    (1161, 0),
    (4347, 1),
    (4447, 2),
    (7467, 1),
    (7521, 0),
    (8369, 1),
    (8426, 0),
    (9000, 1),
    (9002, 2),
    (11021, 1),
    (12350, 2),
    (12351, 1),
    (12438, 2),
    (12442, 0),
    (19893, 2),
    (19967, 1),
    (55203, 2),
    (63743, 1),
    (64106, 2),
    (65039, 1),
    (65059, 0),
    (65131, 2),
    (65279, 1),
    (65376, 2),
    (65500, 1),
    (65510, 2),
    (120831, 1),
    (262141, 2),
    (1114109, 1),
];

async fn request_ping(host: &str) -> Result<u128, Box<dyn Error + Send + Sync>> {
    let command = "PING 0\n";

    let mut stream = TcpStream::connect(&host).await?;
    stream.set_nodelay(true).unwrap();

    let now = Instant::now();
    stream.write_all(command.as_bytes()).await?;

    let mut reader = BufReader::new(stream);
    let mut buffer = String::new();
    reader.read_line(&mut buffer).await?;

    Ok(now.elapsed().as_millis())
}

async fn request_download(
    host: &str,
    rx: &mut Receiver<&str>,
    counter: Arc<Mutex<u128>>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let data_size: u128 = 15 * 1024 * 1024 * 1024;
    let command = format!("DOWNLOAD {}\n", data_size);

    let mut stream = TcpStream::connect(&host).await?;
    stream.set_nodelay(true).unwrap();
    let _r = rx.changed().await.is_ok();

    stream.write_all(command.as_bytes()).await?;

    let mut reader = BufReader::new(stream);

    while !rx.has_changed().unwrap_or(false) {
        reader.read_exact(&mut [0; 16384]).await?;
        let mut num = counter.lock().await;
        *num += 1;
    }
    Ok(true)
}

async fn request_upload(
    host: &str,
    rx: &mut Receiver<&str>,
    counter: Arc<Mutex<u128>>,
) -> Result<bool, Box<dyn Error + Send + Sync>> {
    let data_size: u128 = 15 * 1024 * 1024 * 1024;
    let data = "23456789ABCDEFGHIJKLMNOPQRSTUVWX".repeat(512);
    let data = data.as_bytes();

    let command = format!("UPLOAD {} 0\n", data_size);

    let mut stream = TcpStream::connect(&host).await?;
    let _r = rx.changed().await.is_ok();
    stream.write(command.as_bytes()).await?;

    while !rx.has_changed().unwrap_or(false) {
        stream.write_all(data).await?;
        let mut num = counter.lock().await;
        *num += 1;
    }

    Ok(true)
}

fn get_width(o: u32) -> u8 {
    if o == 0xE || o == 0xF {
        return 0;
    }
    for (num, wid) in WIDTH {
        if o <= num {
            return wid;
        }
    }
    1
}

fn justify_name(name: &String) -> String {
    let mut name_width = 0;
    let mut justified_name = String::new();

    for c in name.chars() {
        let w = get_width(c as u32);
        if name_width + w < 46 {
            name_width += w;
            justified_name.push(c);
        }
    }

    if name_width < 46 {
        let space_count = 46 - name_width;
        justified_name += " ".repeat(space_count as usize).as_str();
    }
    justified_name
}

fn format_size(size: &u128) -> String {
    let num = size * 16384 * 8;
    let mut num = num as f64;
    for unit in ["", "K", "M"] {
        if num < 1000.0 {
            return format!("{:.1} {}bps", num, unit);
        }
        num /= 1000.0;
    }
    return format!("{:.1} Gbps", num);
}

/// Simple program to test network
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Number of thread
    #[clap(short, long, value_parser, default_value_t = 4)]
    thread: u8,
}

struct SpeedtestClient {
    name: String,
    host: String,
    thread: u8,
    result: (u128, u128, u128),
}

impl SpeedtestClient {
    async fn ping(&mut self) -> Result<bool, Box<dyn Error>> {
        let mut count = 10;
        let mut ping_min = 10000;
        while count != 0 {
            let task = request_ping(&self.host);
            let ping_ms = timeout(Duration::from_millis(1000), task)
                .await
                .unwrap_or(Ok(10000))
                .unwrap_or(10000);
            if ping_ms < ping_min {
                ping_min = ping_ms;
            }
            self.result.2 = if ping_min != 10000 {
                ping_min
            } else {
                return Ok(false);
            };
            self.show(false);
            sleep(Duration::from_millis(300)).await;
            count -= 1;
        }
        self.result.2 = if ping_min != 10000 { ping_min } else { 0 };
        self.show(false);
        Ok(true)
    }

    async fn download(&mut self) -> Result<bool, Box<dyn Error>> {
        let (tx, rx) = watch::channel("ready");
        let counter: Arc<Mutex<u128>> = Arc::new(Mutex::new(0));
        let mut all_counter: u128 = 0;

        for _i in 0..self.thread {
            let host = self.host.clone();
            let mut r = rx.clone();
            let c = Arc::clone(&counter);
            tokio::spawn(async move { request_download(&host, &mut r, c).await });
        }

        let now = Instant::now();
        let mut time_used = now.elapsed().as_micros();
        tx.send("downlaod")?;

        while time_used < 15_000_000 {
            sleep(Duration::from_millis(500)).await;
            let num = {
                let mut num = counter.lock().await;
                let n = *num;
                *num = 0;
                n
            };
            time_used = now.elapsed().as_micros();
            all_counter += num;
            self.result.1 = num << 1;
            self.show(false);
        }
        tx.send("stop")?;
        self.result.1 = all_counter / (time_used / 1000_000);
        self.show(false);
        sleep(Duration::from_millis(200)).await;

        Ok(true)
    }

    async fn upload(&mut self) -> Result<bool, Box<dyn Error>> {
        let (tx, rx) = watch::channel("ready");
        let counter: Arc<Mutex<u128>> = Arc::new(Mutex::new(0));
        let mut all_counter: u128 = 0;

        for _i in 0..self.thread {
            let host = self.host.clone();
            let mut r = rx.clone();
            let c = Arc::clone(&counter);
            tokio::spawn(async move { request_upload(&host, &mut r, c).await });
        }

        let now = Instant::now();
        let mut time_used = now.elapsed().as_micros();
        tx.send("upload")?;

        while time_used < 15_000_000 {
            sleep(Duration::from_millis(500)).await;
            let num = {
                let mut num = counter.lock().await;
                let n = *num;
                *num = 0;
                n
            };
            time_used = now.elapsed().as_micros();
            all_counter += num;
            self.result.0 = num << 1;
            self.show(false);
        }
        tx.send("stop")?;
        self.result.0 = all_counter / (time_used / 1000_000);
        self.show(false);
        sleep(Duration::from_millis(200)).await;

        Ok(true)
    }

    fn show(&self, last: bool) {
        let upload = if self.result.0 != 0 {
            format_size(&self.result.0)
        } else {
            if last {
                "Failed".to_string()
            } else {
                "Waiting".to_string()
            }
        };
        let download = if self.result.1 != 0 {
            format_size(&self.result.1)
        } else {
            if last {
                "Failed".to_string()
            } else {
                "Waiting".to_string()
            }
        };
        let ping = if self.result.2 != 0 {
            format!("{}ms", self.result.2)
        } else {
            if last {
                "Failed".to_string()
            } else {
                "Waiting".to_string()
            }
        };

        print!(
            "\r{BOLD}{}{BLUE}{:>12}{ENDC}{RED}{:>12}{ENDC}{GREEN}{:>10}{ENDC}{ENDC}",
            self.name, upload, download, ping
        );
    }

    async fn run(&mut self) -> bool {
        self.show(false);
        let ping = self.ping().await.unwrap_or(false);
        if !ping {
            self.result.2 = 0;
            self.show(true);
            sleep(Duration::from_secs(3)).await;
            return false;
        } else {
            let download = self.download().await.unwrap_or(false);
            if !download {
                self.result.1 = 0;
            } else {
                let upload = self.upload().await.unwrap_or(false);
                if !upload {
                    self.result.0 = 0;
                }
            }
            self.show(true);
            println!("");
        }
        true
    }
}

async fn get_locations() -> Result<Option<Vec<[String; 6]>>, Box<dyn Error>> {
    let url = "https://www.speedtest.net/api/js/servers?engine=js";
    let closest_servers = reqwest::get(url).await?.json::<Value>().await?;

    let s = closest_servers.get(0).unwrap().clone();
    let host = s.get("host").unwrap().as_str().unwrap().to_string();
    let name = s.get("name").unwrap().as_str().unwrap().to_string();
    let country = s.get("country").unwrap().as_str().unwrap().to_string();
    let cc = s.get("cc").unwrap().as_str().unwrap().to_string();
    let sponsor = s.get("sponsor").unwrap().as_str().unwrap().to_string();
    let sid = s.get("id").unwrap().as_str().unwrap().to_string();

    let servers = vec![[host, name, country, cc, sponsor, sid]];
    Ok(Some(servers))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let version = env!("CARGO_PKG_VERSION");
    let line = "-".repeat(80);

    println!("Binet v{}", version);
    println!("{line}");

    let locations = get_locations().await.unwrap();

    println!(
        "{BOLD}{:<46}{:>12}{:>12}{:>10}{ENDC}",
        "Location", "Upload", "Download", "Latency"
    );
    println!("{line}");

    if locations.is_none() {
        println!("{:<80}", "No available servers")
    }

    let mut count_all = 0;
    let mut count_failed = 0;

    let start = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    for location in locations.unwrap_or(vec![]).iter() {
        let name = format!("[{}] {} - {}", location[3], location[4], location[1]);
        let name = justify_name(&name);
        let host: Vec<&str> = location[0].split("//").collect();
        let pos = if host.len() > 1 { host.len() - 1 } else { 0 };
        let host = host[pos].to_string();

        let mut client = SpeedtestClient {
            name: name,
            host: host,
            thread: args.thread,
            result: (0, 0, 0),
        };
        let res = client.run().await;
        if !res {
            count_failed += 1;
        }
        count_all += 1;
    }

    let end = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    if count_all == count_failed {
        println!("\r{:<80}", "All tests failed")
    }
    let res = format!("Passed {}/{}", count_all - count_failed, count_all);
    println!("\r{:-^80}", res);

    print!("Time: {} ~ {}", start, end);
    if args.thread == 1 {
        print!(" Single Thread")
    }
    println!();
}
