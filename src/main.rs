use std::env;
use std::sync::Arc;
use std::time::Duration;

use getopts::Options;
use log::{debug, info};
use tokio::sync::Semaphore;
use tokio::time;

use bim::{add_target_data, get_machine_id, get_targets, test_tcp_pings};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} TOKEN [options]", program);
    print!("{}", opts.usage(&brief));
}

#[tokio::main]
async fn run(token: &str, name: &str) {
    let mut interval = time::interval(Duration::from_secs(300));
    let semaphore = Arc::new(Semaphore::new(4));

    loop {
        info!("Waiting for next tick");
        interval.tick().await;

        let machine_id = match get_machine_id(name, token) {
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

        let targets = match get_targets(token) {
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
            let url = target.url.clone();
            let ipv6 = target.ipv6;
            let s = semaphore.clone();

            let task = tokio::spawn(async move { test_tcp_pings(url, ipv6, s).await });

            tasks.push((target_id, task))
        }

        let mut success = 0;
        for (target_id, task) in tasks {
            if let Ok(Some(data)) = task.await {
                success += 1;
                add_target_data(machine_id, target_id, token, data);
            };
        }

        info!("Finished {success}/{count} targets")
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("n", "name", "set test client name", "NAME");
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

    let token = if matches.free.len() >= 1 {
        matches.free.get(0).unwrap()
    } else {
        print_usage(&program, opts);
        return;
    };

    let name = match matches.opt_str("n") {
        Some(name) => name.to_string(),
        None => "Runner".to_string(),
    };

    env_logger::init();
    debug!("API Token: {token}");
    info!("Running Machine: {name}");

    run(token, &name)
}
