use std::env;
use std::thread;
use std::time::{Duration, SystemTime};

use getopts::Options;
use log::{debug, info};

use bim::{add_target_data, get_machine_id, get_targets, test_tcp_pings};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} TOKEN [options]", program);
    print!("{}", opts.usage(&brief));
}

fn run(token: &str, machine_id: i32) {
    let mut last = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    loop {
        let targets = match get_targets(machine_id) {
            Ok(t) => t,
            Err(e) => {
                info!("Get targets failed: {e}");
                vec![]
            }
        };

        for target in targets {
            let target_id = target.id;
            let url = target.url;
            let ipv6 = target.ipv6;

            if let Some(data) = test_tcp_pings(url, ipv6) {
                match add_target_data(machine_id, target_id, token, data) {
                    Ok(_) => {}
                    Err(e) => info!("Add failed: {e}"),
                }
            };

            thread::sleep(Duration::from_secs(1));
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let time_sleep = 300 - (now - last);

        last = now;
        thread::sleep(Duration::from_secs(time_sleep));
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
    debug!("Name {name} token {token}");

    let machine_id = match get_machine_id(&name, token) {
        Ok(m) => m.id,
        Err(e) => {
            println!("{e}");
            return;
        }
    };

    run(token, machine_id)
}
