//! Bottom bar: real host name, local addresses, and whether the system clock is synced.
//! Nothing here is invented. A missing command becomes an explicit note.

use std::process::Command;

#[derive(Clone, Debug)]
pub struct StatusLine {
    pub hostname: String,
    pub ips: Vec<String>,
    pub sync: String,
    pub alert: String,
}

impl StatusLine {
    pub fn text(&self) -> String {
        let ips = if self.ips.is_empty() {
            "no IP address".to_string()
        } else {
            self.ips.join(", ")
        };
        let body = format!("{}  ·  {}  ·  {}", self.hostname, ips, self.sync);
        if self.alert.trim().is_empty() {
            body
        } else {
            format!("{}  ·  {body}", self.alert.trim())
        }
    }
}

pub fn gather() -> StatusLine {
    StatusLine {
        hostname: read_hostname(),
        ips: read_ips(),
        sync: read_sync(),
        alert: String::new(),
    }
}

fn read_hostname() -> String {
    if let Ok(text) = std::fs::read_to_string("/etc/hostname") {
        let name = text.trim();
        if !name.is_empty() {
            return name.to_string();
        }
    }
    command_stdout("hostname", &[]).unwrap_or_else(|| "unknown host".into())
}

fn read_ips() -> Vec<String> {
    match command_stdout("hostname", &["-I"]) {
        Some(text) => {
            let ips: Vec<String> = text.split_whitespace().map(|s| s.to_string()).collect();
            if ips.is_empty() {
                vec!["no IP address".into()]
            } else {
                ips
            }
        }
        None => vec!["no IP address".into()],
    }
}

fn read_sync() -> String {
    match command_stdout("timedatectl", &["show", "-p", "NTPSynchronized", "--value"]) {
        Some(value) => match value.to_ascii_lowercase().as_str() {
            "yes" => "clock in sync".into(),
            "no" => "clock not synced".into(),
            _ => "time sync unknown".into(),
        },
        None => "time sync unknown".into(),
    }
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}
