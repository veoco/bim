use std::env;
use std::thread;
use std::time::Duration;

use bim_core::clients::{Client, HTTPClient};
use getopts::Options;
use log::{debug, info};

use bim::{finish_task, get_machine_id, get_tasks, get_ip, mask_ipv4};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} TOKEN [options]", program);
    print!("{}", opts.usage(&brief));
}

fn run(token: &str, machine_id: i32) {
    loop {
        let tasks = match get_tasks(machine_id, token) {
            Ok(t) => t,
            Err(e) => {
                info!("Get tasks failed: {e}");
                vec![]
            }
        };

        let tasks_len = tasks.len();

        for task in tasks {
            let server = task.server;
            let server_name = server.name;

            if let Some(mut client) = HTTPClient::build(
                server.download_url,
                server.upload_url,
                server.ipv6,
                server.multi,
            ) {
                let success = client.run();
                if success {
                    let res = client.result();
                    let task_id = task.id;
                    info!("Task {task_id}: {res}");

                    match finish_task(task_id, token, res) {
                        Ok(b) => {
                            if b {
                                info!("Finish {task_id}: success")
                            } else {
                                info!("Finish {task_id}: failed")
                            }
                        }
                        Err(e) => info!("Finish {task_id}: {e}"),
                    }
                }
            } else {
                info!("Server {server_name}: failed")
            }
            thread::sleep(Duration::from_secs(1));
        }

        if tasks_len == 0 {
            thread::sleep(Duration::from_secs(15));
        }
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

    let name = match matches.opt_str("n"){
        Some(name) => name.to_string(),
        None => {
            let ipv4 = match get_ip(false) {
                Ok(ip) => ip,
                Err(e) => {
                    println!("Get ip error: {e}");
                    return;
                }
            };
            mask_ipv4(&ipv4)
        }
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
