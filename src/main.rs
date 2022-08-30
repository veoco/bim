use std::collections::HashMap;
use std::error::Error;
use std::fmt::Debug;

use base64::encode;
use chrono::prelude::*;
use clap::Parser;
use log::{debug, error, info};
use reqwest::header;
use serde_json::Value;
use tokio;
use tokio::time::{interval, Duration};

mod requests;
mod speedtest;
mod task;
mod utils;
mod windows;
use speedtest::SpeedTest;
use task::{get_tasks, register_machine, send_result};
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
    /// Deply mode
    #[clap(short, long, action)]
    deploy: bool,
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

async fn run_once(args: Args) {
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
            false,
        )
        .await;

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

async fn run_forever(email_token: String) {
    let auth = format!("Basic {}", encode(email_token));
    debug!("{}", auth);

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("bim 1"),
    );
    headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&auth).unwrap(),
    );
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();

    let r = register_machine(&client).await;
    let machine_id = match r {
        Ok(mid) => {
            if mid == String::from("") {
                info!("Username or Password error");
                panic!();
            }
            info!("Machine {} registed", mid);
            mid
        }
        Err(_) => {
            error!("Network error");
            panic!();
        }
    };

    let mut time_interval = interval(Duration::from_secs(3600));

    loop {
        time_interval.tick().await;
        let r = get_tasks(&machine_id, &client).await;
        let tasks = match r {
            Ok(tasks) => {
                info!("Fetched {} tasks", tasks.len());
                tasks
            }
            Err(_) => {
                error!("Network error");
                vec![]
            }
        };
        for task in tasks {
            let task_id = task.get("pk").unwrap().to_string();
            let server = task.get("server").unwrap();
            let download_url = server.get("dl").unwrap().as_str().unwrap().to_string();
            let upload_url = server.get("ul").unwrap().as_str().unwrap().to_string();
            let provider = server
                .get("provider")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            let ipv6 = server.get("ipv6").unwrap().as_bool().unwrap();
            let thread = server.get("thread").unwrap().as_u64().unwrap() as u8;

            let speedtest = SpeedTest::build(
                provider,
                "".to_string(),
                download_url,
                upload_url,
                ipv6,
                thread,
                true,
            )
            .await;

            if let Some(mut c) = speedtest {
                info!("Running task {}",  task_id);
                let res = c.run().await;
                if res {
                    info!("Uploading task {} result",  task_id);
                    let _r = send_result(&task_id, &client, &c).await;
                } else {
                    info!("Task {} run failed",  task_id);
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    env_logger::init();
    if args.deploy {
        info!("Enter deploy mode");
        run_forever(args.server).await;
    } else {
        info!("Enter oneshot mode");
        run_once(args).await;
    }
}
