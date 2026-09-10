use std::net::{Ipv4Addr, UdpSocket};

/// Validates that an IPv4 address is an actual assigned local network address,
/// not loopback (127.0.0.0/8), unspecified (0.0.0.0), or link-local auto-ip (169.254.0.0/16).
fn is_valid_local_ipv4(ip: &Ipv4Addr) -> bool {
    !ip.is_unspecified()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_broadcast()
        && !ip.is_multicast()
}

/// Dynamically discovers the active local IPv4 address without hardcoded IP dependencies.
/// Uses the OS routing table via dummy UDP socket connection (no network packets are transmitted).
pub fn get_local_ipv4() -> String {
    // 1. Primary: Public DNS servers.
    // Querying the OS routing table for a global destination resolves the active default gateway interface
    // automatically across ANY subnet (10.x.x.x, 172.16-31.x.x, 192.168.x.x, etc.).
    let primary_targets = [
        "8.8.8.8:80",        // Google DNS
        "1.1.1.1:80",        // Cloudflare DNS
        "208.67.222.222:80", // OpenDNS
        "9.9.9.9:80",        // Quad9
    ];

    for target in primary_targets {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(target).is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    if let std::net::IpAddr::V4(ipv4) = addr.ip() {
                        if is_valid_local_ipv4(&ipv4) {
                            return ipv4.to_string();
                        }
                    }
                }
            }
        }
    }

    // 2. Secondary fallback: For completely isolated offline LANs without an external gateway,
    // probe private subnet broadcast addresses to determine the local active adapter.
    let subnet_targets = [
        "10.255.255.255:80",  // Class A private subnet
        "172.31.255.255:80",  // Class B private subnet
        "192.168.255.255:80", // Class C private subnet
        "192.168.1.1:80",
        "192.168.0.1:80",
        "10.0.0.1:80",
    ];

    for target in subnet_targets {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(target).is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    if let std::net::IpAddr::V4(ipv4) = addr.ip() {
                        if is_valid_local_ipv4(&ipv4) {
                            return ipv4.to_string();
                        }
                    }
                }
            }
        }
    }

    "127.0.0.1".to_string()
}
