use std::env;

use getopts::Options;

use bim_core::servers::{HTTPServer, Server};

use bim::{create_server, get_ip, ServerData};

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} TOKEN [options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("n", "name", "set test server name", "NAME");
    opts.optopt("p", "port", "set test server port", "PORT");
    opts.optflag("6", "ipv6", "enable ipv6 mode");
    opts.optflag("m", "multi", "enable multi thread mode");
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

    #[cfg(debug_assertions)]
    env_logger::init();

    let port = matches.opt_str("p").unwrap_or("8388".to_string());
    let address = format!("0.0.0.0:{port}");

    let ipv6 = matches.opt_present("6");

    if let Some(mut server) = HTTPServer::build(address.to_string()) {
        let ip = match get_ip(ipv6) {
            Ok(ip) => ip,
            Err(e) => {
                println!("Get ip error: {e}");
                return;
            }
        };

        let server_name = matches.opt_str("n").unwrap_or(ip.clone());
        let url = format!("http://{ip}:{port}/speedtest");

        let data = ServerData {
            name: server_name.clone(),
            token: token.to_string(),
            ipv6: matches.opt_present("6"),
            multi: matches.opt_present("m"),
            download_url: url.clone(),
            upload_url: url.clone(),
        };
        match create_server(data) {
            Ok(_) => {}
            Err(e) => {
                println!("Create server error: {e}");
                return;
            }
        }

        println!("Running {server_name} server on: {address}, url {url}");
        let _ = server.run();
    } else {
        println!("Invalid params.")
    }
}
