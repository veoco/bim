use std::env;
use std::sync::Arc;
use std::time::Duration;

use getopts::Options;
use log::{debug, info};
use tokio::sync::Semaphore;
use tokio::time;

use bim::{add_target_data, get_machine_id, get_targets, ping};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("n", "name", "set test client name", "NAME");
    opts.optopt("t", "token", "set the token", "TOKEN");
    opts.optopt("s", "server_url", "set the server URL", "SERVER_URL");
    opts.optflag("h", "help", "print this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("{}\n", f.to_string());
            print_usage(&program, opts);
            return;
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, opts);
        return;
    }

    let name = match matches.opt_str("n") {
        Some(name) => name,
        None => {
            print_usage(&program, opts);
            return;
        }
    };

    let token = match matches.opt_str("t") {
        Some(t) => t,
        None => {
            print_usage(&program, opts);
            return;
        }
    };

    let server_url = match matches.opt_str("s") {
        Some(s) => s,
        None => {
            print_usage(&program, opts);
            return;
        }
    };

    env_logger::init();
    debug!("API Token: {token}");
    info!("Running Machine: {name}");

    run(name, token, server_url);
}

#[tokio::main]
async fn run(name: String, token: String, server_url: String) {
    let mut interval = time::interval(Duration::from_secs(300));
    let semaphore = Arc::new(Semaphore::new(8));

    loop {
        info!("Waiting for next tick");
        interval.tick().await;

        let machine_id = match get_machine_id(&name, &token, &server_url) {
            Ok(m) => {
                let mid = m.id;
                info!("Machine id: {mid}");
                mid
            }
            Err(e) => {
                info!("Get machine id failed: {e}");
                continue;
            }
        };

        let targets = match get_targets(&token, &server_url) {
            Ok(t) => t,
            Err(e) => {
                info!("Get targets failed: {e}");
                continue;
            }
        };

        let count = targets.len();
        info!("Testing {count} targets");

        let mut tasks = vec![];

        for target in targets {
            let target_id = target.id;

            if target.domain.is_some() {
                let domain = target.domain.unwrap();

                let target = domain.clone();
                let s = semaphore.clone();
                let task = tokio::spawn(async move { ping(target, false, s).await });
                tasks.push((target_id, task));

                let s = semaphore.clone();
                let target = domain.clone();
                let task = tokio::spawn(async move { ping(target, true, s).await });
                tasks.push((target_id, task));
            } else {
                if let Some(ipv4) = target.ipv4 {
                    let s = semaphore.clone();
                    let task = tokio::spawn(async move { ping(ipv4, false, s).await });
                    tasks.push((target_id, task));
                }

                if let Some(ipv6) = target.ipv6 {
                    let s = semaphore.clone();
                    let task = tokio::spawn(async move { ping(ipv6, true, s).await });
                    tasks.push((target_id, task));
                }
            }
        }

        let task_count = tasks.len();

        for (target_id, task) in tasks {
            if let Ok(Some(data)) = task.await {
                let token = token.clone();
                let server_url = server_url.clone();
                tokio::spawn(async move {
                    add_target_data(machine_id, target_id, &token, &server_url, data)
                });
            };
        }

        info!("Finished {task_count} tasks")
    }
}
