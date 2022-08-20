use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;

use chrono::prelude::*;
use clap::Parser;
use serde_json::Value;
use tokio;

mod requests;
mod speedtest;
mod utils;
mod windows;
use speedtest::SpeedTest;
use utils::{justify_name, BOLD, ENDC};

/// Simple program to test network
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(value_parser)]
    server: String,
    /// Enable server list search
    #[clap(short, long, action)]
    server_list: bool,
    /// Enable IPv6 only test
    #[clap(short = '6', long, action)]
    ipv6: bool,
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
        let mut name = detail.get("name").unwrap().as_str().unwrap().to_string();
        let mut r = HashMap::new();

        r.insert(String::from("provider"), provider.clone());
        r.insert(
            String::from("ipv6"),
            detail.get("ipv6").unwrap().to_string(),
        );
        r.insert(
            String::from("dl"),
            detail.get("dl").unwrap().as_str().unwrap().to_string(),
        );
        r.insert(
            String::from("ul"),
            detail.get("ul").unwrap().as_str().unwrap().to_string(),
        );

        if provider == "Ookla" {
            let cc = detail.get("cc").unwrap().as_str().unwrap().to_string();
            let sponsor = detail.get("sponsor").unwrap().as_str().unwrap().to_string();

            name = format!("[{}] {} - {}", cc, sponsor, name);
            name = justify_name(&name);
        } else if provider == "LibreSpeed" {
            let sponsor_name = detail
                .get("sponsorName")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();

            name = format!("{} - {}", sponsor_name, name);
            name = justify_name(&name);
        }
        r.insert(String::from("name"), name);

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
        let provider = location.get("provider").unwrap().clone();

        let ipv6 = location.get("ipv6").unwrap();
        let ipv6 = if ipv6 == "false" { false } else { true };
        if args.ipv6 {
            if !ipv6 {
                continue;
            }
        }

        let name = location.get("name").unwrap().clone();
        let download_url = location.get("dl").unwrap().clone();
        let upload_url = location.get("ul").unwrap().clone();

        let client = SpeedTest::build(
            provider,
            name,
            download_url,
            upload_url,
            if args.ipv6 && ipv6 { true } else { false },
            args.thread,
        ).await;

        if let Some(mut c) = client {
            let res = c.run().await;
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
