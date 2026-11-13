//! Bottom bar: hostname, the IPv4 on the default-route interface, the crate version,
//! and a warning only when something is actually wrong.
//!
//! Nothing here is invented. A missing address or a missing clock-sync check becomes
//! a short note, not a list of every interface.

use std::net::Ipv4Addr;
use std::path::Path;
use std::process::Command;

const NETWORK: char = '\u{1F5A7}';

#[derive(Clone, Debug)]
pub struct StatusLine {
    pub hostname: String,
    /// IPv4 of the interface that carries the default route. None when it cannot be read.
    pub address: Option<String>,
    /// Clock-sync problem, empty when the clock is in sync or the check is not a failure.
    pub warning: String,
    /// A real fault from elsewhere (a failed weather fetch). Empty when nothing is wrong.
    pub alert: String,
}

impl StatusLine {
    /// Host, address, and any real fault. The version is painted on the right.
    pub fn left_text(&self) -> String {
        let address = self.address.as_deref().unwrap_or("—");
        let mut line = format!("{host} {NETWORK} {address}", host = self.hostname);
        for extra in [&self.warning, &self.alert] {
            let extra = extra.trim();
            if !extra.is_empty() {
                line.push_str("  ");
                line.push_str(extra);
            }
        }
        line
    }

    pub fn version_text() -> String {
        format!("v{}", env!("CARGO_PKG_VERSION"))
    }

    pub fn text(&self) -> String {
        let address = self.address.as_deref().unwrap_or("—");
        let mut line = format!(
            "{host} {NETWORK} {address}  {}",
            Self::version_text(),
            host = self.hostname,
        );
        for extra in [&self.warning, &self.alert] {
            let extra = extra.trim();
            if !extra.is_empty() {
                line.push_str("  ");
                line.push_str(extra);
            }
        }
        line
    }
}

pub fn gather() -> StatusLine {
    let route = std::fs::read_to_string("/proc/net/route").unwrap_or_default();
    let fib = std::fs::read_to_string("/proc/net/fib_trie").unwrap_or_default();
    StatusLine {
        hostname: read_hostname(),
        address: primary_ipv4(&route, &fib),
        warning: sync_warning().unwrap_or_default(),
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

/// `Some` only when timedatectl reports a problem, or when the check itself cannot be made.
/// A synced clock adds nothing to the line.
fn sync_warning() -> Option<String> {
    let value = command_stdout("timedatectl", &["show", "-p", "NTPSynchronized", "--value"]);
    sync_warning_from(value.as_deref())
}

fn sync_warning_from(value: Option<&str>) -> Option<String> {
    match value.map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("yes") => None,
        Some("no") => Some("clock not synced".into()),
        _ => Some("time sync unknown".into()),
    }
}

/// IPv4 on the interface used for the default route.
///
/// Loopback, docker, bridges, and tunnels are skipped, so a docker bridge address
/// is not reported when a real interface holds the default route. The first
/// non-link-local address on that interface wins.
fn primary_ipv4(route_text: &str, fib_text: &str) -> Option<String> {
    let routes = parse_routes(route_text);
    let locals = local_host_addrs(fib_text);
    let mut defaults: Vec<&Route> = routes
        .iter()
        .filter(|r| r.destination.is_unspecified())
        .collect();
    defaults.sort_by_key(|r| r.metric);
    for route in defaults {
        if skip_iface(&route.iface) {
            continue;
        }
        if let Some(addr) = address_on(&route.iface, &routes, &locals) {
            return Some(addr.to_string());
        }
    }
    None
}

fn address_on(iface: &str, routes: &[Route], locals: &[Ipv4Addr]) -> Option<Ipv4Addr> {
    let mut found = None;
    for route in routes {
        if route.iface != iface || route.mask.is_unspecified() || route.destination.is_unspecified()
        {
            continue;
        }
        for addr in locals {
            if addr.is_loopback()
                || addr.is_link_local()
                || addr.is_broadcast()
                || addr.is_unspecified()
            {
                continue;
            }
            if !in_subnet(*addr, route.destination, route.mask) {
                continue;
            }
            if *addr == route.destination {
                continue;
            }
            found = Some(*addr);
            break;
        }
        if found.is_some() {
            break;
        }
    }
    found
}

fn in_subnet(addr: Ipv4Addr, network: Ipv4Addr, mask: Ipv4Addr) -> bool {
    let mask = u32::from(mask);
    u32::from(addr) & mask == u32::from(network) & mask
}

struct Route {
    iface: String,
    destination: Ipv4Addr,
    mask: Ipv4Addr,
    metric: u32,
}

fn parse_routes(text: &str) -> Vec<Route> {
    let mut routes = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split_whitespace().collect();
        // Iface Destination Gateway Flags RefCnt Use Metric Mask ...
        if cols.len() < 8 {
            continue;
        }
        let Ok(destination) = parse_proc_ip(cols[1]) else {
            continue;
        };
        let Ok(mask) = parse_proc_ip(cols[7]) else {
            continue;
        };
        let metric = cols[6].parse().unwrap_or(0);
        routes.push(Route {
            iface: cols[0].to_string(),
            destination,
            mask,
            metric,
        });
    }
    routes
}

/// `/proc/net/route` prints IPv4 as a native-endian hex word.
fn parse_proc_ip(hex: &str) -> Result<Ipv4Addr, ()> {
    let value = u32::from_str_radix(hex, 16).map_err(|_| ())?;
    Ok(Ipv4Addr::from(value.to_le_bytes()))
}

fn local_host_addrs(fib: &str) -> Vec<Ipv4Addr> {
    let mut addrs = Vec::new();
    let mut pending = None;
    for line in fib.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("|-- ") {
            pending = rest.parse().ok();
        } else if trimmed.contains("host LOCAL") {
            if let Some(addr) = pending.take() {
                addrs.push(addr);
            }
        }
    }
    addrs
}

fn skip_iface(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if lower == "lo" {
        return true;
    }
    const PREFIXES: &[&str] = &[
        "docker",
        "br-",
        "br",
        "veth",
        "virbr",
        "tun",
        "tap",
        "wg",
        "ppp",
        "tailscale",
        "zt",
        "cni",
        "flannel",
        "dummy",
        "ifb",
        "vboxnet",
        "vmnet",
        "kube",
        "cali",
    ];
    if PREFIXES.iter().any(|prefix| lower.starts_with(prefix)) {
        return true;
    }
    let base = Path::new("/sys/class/net").join(name);
    base.join("bridge").exists() || base.join("tun_flags").exists()
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

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTE: &str = "\
Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT
enp0s3\t00000000\t01001EAC\t0003\t0\t0\t100\t00000000\t0\t0\t0
docker0\t000011AC\t00000000\t0001\t0\t0\t0\t0000FFFF\t0\t0\t0
enp0s3\t00001EAC\t00000000\t0001\t0\t0\t0\t00FFFFFF\t0\t0\t0
";

    const FIB: &str = "\
Local:
  |-- 127.0.0.1
     /32 host LOCAL
  |-- 172.17.0.1
     /32 host LOCAL
  |-- 172.30.0.2
     /32 host LOCAL
";

    #[test]
    fn primary_address_is_the_default_route_not_docker() {
        assert_eq!(primary_ipv4(ROUTE, FIB).as_deref(), Some("172.30.0.2"));
    }

    #[test]
    fn a_docker_default_route_is_skipped() {
        let route = "\
Iface Destination Gateway Flags RefCnt Use Metric Mask
docker0 00000000 010011AC 0003 0 0 0 00000000
enp0s3 00000000 01001EAC 0003 0 0 200 00000000
enp0s3 00001EAC 00000000 0001 0 0 0 00FFFFFF
";
        assert_eq!(primary_ipv4(route, FIB).as_deref(), Some("172.30.0.2"));
    }

    #[test]
    fn missing_route_has_no_address() {
        assert_eq!(primary_ipv4("", FIB), None);
    }

    #[test]
    fn line_is_host_mark_address_and_version() {
        let line = StatusLine {
            hostname: "Tower5810".into(),
            address: Some("10.0.0.113".into()),
            warning: String::new(),
            alert: String::new(),
        };
        assert_eq!(
            line.text(),
            format!(
                "Tower5810 \u{1F5A7} 10.0.0.113  v{}",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn a_sync_problem_is_appended_and_a_healthy_clock_is_not() {
        assert_eq!(sync_warning_from(Some("yes")), None);
        assert_eq!(
            sync_warning_from(Some("no")).as_deref(),
            Some("clock not synced")
        );
        assert_eq!(
            sync_warning_from(None).as_deref(),
            Some("time sync unknown")
        );
        let line = StatusLine {
            hostname: "Tower5810".into(),
            address: Some("10.0.0.113".into()),
            warning: "time sync unknown".into(),
            alert: String::new(),
        };
        assert_eq!(
            line.text(),
            format!(
                "Tower5810 \u{1F5A7} 10.0.0.113  v{}  time sync unknown",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn a_missing_address_is_a_short_mark() {
        let line = StatusLine {
            hostname: "Tower5810".into(),
            address: None,
            warning: String::new(),
            alert: String::new(),
        };
        assert_eq!(
            line.text(),
            format!("Tower5810 \u{1F5A7} —  v{}", env!("CARGO_PKG_VERSION"))
        );
    }
}
