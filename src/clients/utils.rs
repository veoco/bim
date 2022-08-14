use std::io::prelude::*;

use crate::utils::{format_size, BLUE, BOLD, ENDC, GREEN, RED};

pub trait Speedtest {
    fn get_upload(&self) -> u128;

    fn get_download(&self) -> u128;

    fn get_name(&self) -> &str;

    fn get_ping(&self) -> u128;

    fn show(&self) {
        let upload = format_size(self.get_upload());

        let download = format_size(self.get_download());

        let ping = self.get_ping();
        let ping = if ping == 0 {
            String::from("-")
        } else {
            if ping < 1000 {
                format!("<1 ms")
            } else {
                format!("{:.1} ms", ping as f64 / 1000.0)
            }
        };

        let name = self.get_name();

        let line = format!(
            "\r{BOLD}{}{BLUE}{:>12}{ENDC}{RED}{:>12}{ENDC}{GREEN}{:>10}{ENDC}{ENDC}",
            name, upload, download, ping
        );
        let mut stdout = std::io::stdout();
        let _r = stdout.write_all(line.as_bytes());
        let _r = stdout.flush();
    }
}
