use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;

use chrono::prelude::*;
use clap::Parser;
use serde_json::Value;
use tokio;

mod clients;
mod utils;
mod windows;
use clients::librespeed_org::LibreSpeedOrgClient;
use clients::speedtest_net::SpeedtestNetClient;
use utils::{justify_name, BOLD, ENDC};

/// Simple program to test network
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser)]
    server: String,
    /// Server list search
    #[clap(short, long, action)]
    server_list: bool,
    /// Number of thread
    #[clap(short, long, value_parser, default_value_t = 4)]
    thread: u8,
}

async fn get_servers(args: &Args) -> Result<Option<Vec<HashMap<String, String>>>, Box<dyn Error>> {
    let server = &args.server;
    let servers: Vec<Value>;
    if args.server_list {
        let url = format!("https://bench.im/api/server_list/?pk={}", server);
        servers = reqwest::get(url)
            .await?
            .json::<Value>()
            .await?
            .get("servers")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
    } else {
        let url = format!("https://bench.im/api/search/?type=server&query={}", server);
        servers = reqwest::get(url)
            .await?
            .json::<Value>()
            .await?
            .get("results")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
    }

    let mut results = vec![];
    for s in servers {
        let provider = s.get("provider").unwrap().as_str().unwrap().to_string();
        let detail = s.get("detail").unwrap();
        let mut r = HashMap::new();
        r.insert(String::from("provider"), provider.clone());

        if provider == "Ookla" {
            let host = detail.get("host").unwrap().as_str().unwrap().to_string();
            let name = detail.get("name").unwrap().as_str().unwrap().to_string();
            let cc = detail.get("cc").unwrap().as_str().unwrap().to_string();
            let sponsor = detail.get("sponsor").unwrap().as_str().unwrap().to_string();

            let name = format!("[{}] {} - {}", cc, sponsor, name);
            let name = justify_name(&name);

            r.insert(String::from("name"), name);
            r.insert(String::from("host"), host);
        } else if provider == "LibreSpeed" {
            let mut server = detail.get("server").unwrap().as_str().unwrap().to_string();
            let name = detail.get("name").unwrap().as_str().unwrap().to_string();
            let dl_url = detail.get("dlURL").unwrap().as_str().unwrap().to_string();
            let ul_url = detail.get("ulURL").unwrap().as_str().unwrap().to_string();
            let sponsor_name = detail
                .get("sponsorName")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();

            let name = format!("{} - {}", sponsor_name, name);
            let name = justify_name(&name);
            if server.starts_with("//") {
                server = String::from("https:") + &server;
            }
            if !server.ends_with("/") {
                server = server + &String::from("/");
            }
            let download_url = server.clone() + &dl_url;
            let upload_url = server.clone() + &ul_url;

            r.insert(String::from("name"), name);
            r.insert(String::from("download_url"), download_url);
            r.insert(String::from("upload_url"), upload_url);
        }

        results.push(r);
    }
    Ok(Some(results))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let version = env!("CARGO_PKG_VERSION");
    let line = "-".repeat(80);

    let _enable = windows::enable_ansi_support();
    println!("Bench.im v{}", version);
    println!("{line}");

    let locations = get_servers(&args).await.unwrap();

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
        let provider = location.get("provider").unwrap();

        if provider == "Ookla" {
            let name = location.get("name").unwrap();
            let host = location.get("host").unwrap();
            let mut client = SpeedtestNetClient {
                name: name.clone(),
                host: host.clone(),
                thread: args.thread,
                result: (0, 0, 0),
            };
            let res = client.run().await;
            if !res {
                count_failed += 1;
            }
        } else if provider == "LibreSpeed" {
            let name = location.get("name").unwrap();
            let download_url = location.get("download_url").unwrap();
            let upload_url = location.get("upload_url").unwrap();
            let mut client = LibreSpeedOrgClient {
                name: name.clone(),
                download_url: download_url.clone(),
                upload_url: upload_url.clone(),
                thread: args.thread,
                result: (0, 0, 0),
            };
            let res = client.run().await;
            if !res {
                count_failed += 1;
            }
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
