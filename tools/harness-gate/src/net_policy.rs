use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Normalize a host for exact allowlist comparison. A trailing DNS root dot is
/// insignificant, while wildcard entries remain invalid by design.
pub(crate) fn normalize_host(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

pub(crate) fn valid_allowlist_host(host: &str) -> bool {
    let normalized = normalize_host(host);
    !normalized.is_empty()
        && normalized == host.trim_end_matches('.').to_ascii_lowercase()
        && !normalized.contains('*')
        && !normalized.chars().any(|character| {
            character.is_ascii_whitespace()
                || character.is_ascii_control()
                || matches!(character, '/' | '\\' | '@' | '?')
        })
        && (normalized.parse::<IpAddr>().is_ok()
            || normalized
                .split('.')
                .all(|label| !label.is_empty() && label.bytes().all(is_host_byte)))
}

fn is_host_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
}

/// Local-only ranges are denied for every resolved address. This includes
/// IPv4-mapped IPv6 addresses and the IPv6 unique-local/link-local ranges.
pub(crate) fn is_local_only(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_local_ipv4(address),
        IpAddr::V6(address) => is_local_ipv6(address),
    }
}

fn is_local_ipv4(address: Ipv4Addr) -> bool {
    address.is_unspecified()
        || address.is_loopback()
        || address.is_private()
        || address.is_link_local()
        || address.is_broadcast()
        || address.is_multicast()
        || address.octets()[0] == 0
}

fn is_local_ipv6(address: Ipv6Addr) -> bool {
    address.is_unspecified()
        || address.is_loopback()
        || address.is_multicast()
        || (address.segments()[0] & 0xfe00) == 0xfc00
        || (address.segments()[0] & 0xffc0) == 0xfe80
        || address.to_ipv4_mapped().is_some_and(is_local_ipv4)
}

#[cfg(test)]
mod tests {
    use super::{is_local_only, normalize_host, valid_allowlist_host};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn normalizes_exact_hosts_without_wildcards() {
        assert_eq!(normalize_host("Hooks.Example.TEST."), "hooks.example.test");
        assert!(valid_allowlist_host("hooks.example.test"));
        assert!(valid_allowlist_host("127.0.0.1"));
        assert!(!valid_allowlist_host("*.example.test"));
        assert!(!valid_allowlist_host("https://example.test"));
    }

    #[test]
    fn rejects_local_address_matrix() {
        for address in [
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1)),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V4(Ipv4Addr::BROADCAST),
            IpAddr::V4(Ipv4Addr::new(0, 12, 34, 56)),
            IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1)),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::LOCALHOST),
            IpAddr::V6("fc00::1".parse().expect("unique local address")),
            IpAddr::V6("fe80::1".parse().expect("link local address")),
            IpAddr::V6("ff00::1".parse().expect("multicast address")),
            IpAddr::V6("::ffff:127.0.0.1".parse().expect("mapped loopback")),
        ] {
            assert!(is_local_only(address), "expected local address: {address}");
        }
        assert!(!is_local_only(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_local_only(
            "2001:4860:4860::8888"
                .parse::<IpAddr>()
                .expect("public IPv6 address")
        ));
    }
}
