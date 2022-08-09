use std::error::Error;
use std::fmt::Debug;

use chrono::prelude::*;
use clap::Parser;
use serde_json::Value;
use tokio;

mod client;
mod utils;
use client::SpeedtestClient;
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

async fn get_servers(args: &Args) -> Result<Option<Vec<[String; 4]>>, Box<dyn Error>> {
    let server = &args.server;
    let servers: Vec<Value>;
    if args.server_list {
        let url = format!(
            "https://bench.im/api/server_list/?pk={}",
            server
        );
        servers = reqwest::get(url).await?.json::<Value>().await?.get("servers").unwrap().as_array().unwrap().clone();
    } else {
        let url = format!("https://bench.im/api/search/?type=server&query={}", server);
        servers = reqwest::get(url).await?.json::<Value>().await?.get("results").unwrap().as_array().unwrap().clone();
    }

    let mut results = vec![];
    for s in servers {
        let detial = s.get("detail").unwrap();
        let host = detial.get("host").unwrap().as_str().unwrap().to_string();
        let name = detial.get("name").unwrap().as_str().unwrap().to_string();
        let cc = detial.get("cc").unwrap().as_str().unwrap().to_string();
        let sponsor = detial.get("sponsor").unwrap().as_str().unwrap().to_string();
        results.push([host, name, cc, sponsor])
    }
    Ok(Some(results))
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let version = env!("CARGO_PKG_VERSION");
    let line = "-".repeat(80);

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
        let name = format!("[{}] {} - {}", location[2], location[3], location[1]);
        let name = justify_name(&name);
        let host = location[0].clone();

        let upload_data = "1234567".repeat(128);

        let mut client = SpeedtestClient {
            name: name,
            host: host,
            thread: args.thread,
            result: (0, 0, 0),
            upload_data: upload_data,
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
