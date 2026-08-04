use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::error::ToolError;
use crate::registry::Tool;
use crate::spec::{Category, Field, ToolSpec};
use crate::value::{Inputs, Outputs};

pub struct IpTool {
    spec: ToolSpec,
}

impl Default for IpTool {
    fn default() -> Self {
        Self {
            spec: ToolSpec::new("web.ip", "IP Subnet", Category::Web)
                .describe("Break a CIDR block into network, mask, range, and host count")
                .keywords(&[
                    "ip",
                    "cidr",
                    "subnet",
                    "netmask",
                    "network",
                    "ipv4",
                    "ipv6",
                    "mask",
                    "broadcast",
                    "range",
                ])
                .input(
                    Field::text("cidr")
                        .mono()
                        .label("Address / CIDR")
                        .help("`192.168.1.10/24`, `10.0.0.1`, or `2001:db8::1/48`"),
                )
                .output(Field::text("version").label("Version"))
                .output(Field::text("network").mono().label("Network"))
                .output(Field::text("netmask").mono().label("Netmask"))
                .output(Field::text("wildcard").mono().label("Wildcard"))
                .output(Field::text("broadcast").mono().label("Broadcast"))
                .output(Field::text("range").mono().label("Address range"))
                .output(Field::text("first_host").mono().label("First host"))
                .output(Field::text("last_host").mono().label("Last host"))
                .output(Field::text("total").label("Total addresses"))
                .output(Field::text("usable").label("Usable hosts"))
                .output(Field::text("scope").label("Scope")),
        }
    }
}

/// Both families are handled as a single unsigned integer plus a bit width, so the
/// masking arithmetic is written once. Only the rendering and the host-count
/// conventions differ, and those differ genuinely rather than incidentally.
fn to_bits(ip: IpAddr) -> u128 {
    match ip {
        IpAddr::V4(v4) => u128::from(u32::from(v4)),
        IpAddr::V6(v6) => u128::from(v6),
    }
}

fn from_bits(bits: u128, v4: bool) -> IpAddr {
    if v4 {
        IpAddr::V4(Ipv4Addr::from(bits as u32))
    } else {
        IpAddr::V6(Ipv6Addr::from(bits))
    }
}

/// A mask of `prefix` leading ones in a `width`-bit space. Written with a shift on the
/// *inverse* because `u128 << 128` is an overflow, and `/0` is a legal prefix.
fn mask_of(prefix: u32, width: u32) -> u128 {
    if prefix == 0 {
        0
    } else {
        (!0u128 >> (128 - prefix)) << (width - prefix)
    }
}

fn scope_of(ip: IpAddr) -> &'static str {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            if v4.is_loopback() {
                "loopback"
            } else if v4.is_private() {
                "private"
            } else if v4.is_link_local() {
                "link-local"
            } else if v4.is_multicast() {
                "multicast"
            } else if v4.is_unspecified() {
                "unspecified"
            } else if v4.is_broadcast() {
                "broadcast"
            // Carrier-grade NAT (RFC 6598) and the reserved 240/4 block have no stable
            // std predicate, so they're spelled out rather than reported as "public".
            } else if a == 100 && (64..128).contains(&b) {
                "shared (CGNAT)"
            } else if a >= 240 {
                "reserved"
            } else {
                "public"
            }
        }
        IpAddr::V6(v6) => {
            let segments = v6.segments();
            if v6.is_loopback() {
                "loopback"
            } else if v6.is_unspecified() {
                "unspecified"
            } else if v6.is_multicast() {
                "multicast"
            } else if segments[0] & 0xfe00 == 0xfc00 {
                "unique local"
            } else if segments[0] & 0xffc0 == 0xfe80 {
                "link-local"
            } else if segments[0] == 0x2001 && segments[1] == 0x0db8 {
                "documentation"
            } else {
                "global"
            }
        }
    }
}

/// `2^host_bits` as a decimal string. A full `::/0` is `2^128`, which does not fit in a
/// `u128` — and since the value is only ever displayed, computing it as text sidesteps
/// the overflow instead of capping the answer at `u128::MAX`.
fn pow2(host_bits: u32) -> String {
    if host_bits >= 128 {
        "340282366920938463463374607431768211456".to_string()
    } else {
        (1u128 << host_bits).to_string()
    }
}

impl Tool for IpTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn run(&self, i: &Inputs) -> Result<Outputs, ToolError> {
        let input = i.text("cidr").trim();
        if input.is_empty() {
            return Err(ToolError::invalid("cidr", "address must not be empty"));
        }

        let (addr_part, prefix_part) = match input.split_once('/') {
            Some((a, p)) => (a.trim(), Some(p.trim())),
            None => (input, None),
        };

        let ip: IpAddr = addr_part.parse().map_err(|_| {
            ToolError::invalid("cidr", format!("`{addr_part}` is not an IP address"))
        })?;
        let is_v4 = ip.is_ipv4();
        let width: u32 = if is_v4 { 32 } else { 128 };

        // A bare address is its own /32 (or /128) — the tool then reports what that
        // single address *is*, which is a useful answer rather than an error.
        let prefix = match prefix_part {
            Some(p) => {
                let n: u32 = p.parse().map_err(|_| {
                    ToolError::invalid("cidr", format!("`{p}` is not a prefix length"))
                })?;
                if n > width {
                    return Err(ToolError::invalid(
                        "cidr",
                        format!(
                            "prefix /{n} is out of range for IPv{} (max /{width})",
                            if is_v4 { 4 } else { 6 }
                        ),
                    ));
                }
                n
            }
            None => width,
        };

        let mask = mask_of(prefix, width);
        let bits = to_bits(ip);
        let network = bits & mask;
        let last = network | !mask & mask_of(width, width);
        let host_bits = width - prefix;

        let net_ip = from_bits(network, is_v4);
        let last_ip = from_bits(last, is_v4);

        // Host conventions are an IPv4 thing: the network and broadcast addresses are
        // not assignable, except in the /31 point-to-point (RFC 3021) and /32 cases
        // where both ends are. IPv6 has no broadcast address and no such carve-out.
        let (first_host, last_host, usable) = if !is_v4 || host_bits <= 1 {
            (net_ip, last_ip, pow2(host_bits))
        } else {
            (
                from_bits(network + 1, is_v4),
                from_bits(last - 1, is_v4),
                ((1u128 << host_bits) - 2).to_string(),
            )
        };

        let mut out = Outputs::new();
        out.set("version", if is_v4 { "IPv4" } else { "IPv6" });
        out.set("network", format!("{net_ip}/{prefix}"));
        out.set("netmask", from_bits(mask, is_v4).to_string());
        out.set(
            "wildcard",
            from_bits(!mask & mask_of(width, width), is_v4).to_string(),
        );
        out.set(
            "broadcast",
            if is_v4 {
                last_ip.to_string()
            } else {
                "n/a (IPv6 has no broadcast)".to_string()
            },
        );
        out.set("range", format!("{net_ip} – {last_ip}"));
        out.set("first_host", first_host.to_string());
        out.set("last_host", last_host.to_string());
        out.set("total", pow2(host_bits));
        out.set("usable", usable);
        out.set("scope", scope_of(net_ip));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cidr: &str) -> Result<Outputs, ToolError> {
        IpTool::default().run(&Inputs::new().with("cidr", cidr))
    }

    fn field(cidr: &str, key: &str) -> String {
        run(cidr).unwrap().get(key).unwrap().as_display()
    }

    #[test]
    fn a_host_address_is_masked_down_to_its_network() {
        assert_eq!(field("192.168.1.130/24", "network"), "192.168.1.0/24");
        assert_eq!(field("192.168.1.130/24", "netmask"), "255.255.255.0");
        assert_eq!(field("192.168.1.130/24", "wildcard"), "0.0.0.255");
        assert_eq!(field("192.168.1.130/24", "broadcast"), "192.168.1.255");
        assert_eq!(field("192.168.1.130/24", "first_host"), "192.168.1.1");
        assert_eq!(field("192.168.1.130/24", "last_host"), "192.168.1.254");
        assert_eq!(field("192.168.1.130/24", "total"), "256");
        assert_eq!(field("192.168.1.130/24", "usable"), "254");
    }

    #[test]
    fn odd_prefixes_are_handled() {
        assert_eq!(field("10.0.0.0/12", "netmask"), "255.240.0.0");
        assert_eq!(field("10.5.6.7/12", "network"), "10.0.0.0/12");
        assert_eq!(field("10.0.0.0/12", "usable"), "1048574");
    }

    /// `/0` and `/32` are the two ends that overflow naive shift arithmetic.
    #[test]
    fn the_extreme_prefixes_do_not_overflow() {
        assert_eq!(field("0.0.0.0/0", "netmask"), "0.0.0.0");
        assert_eq!(field("0.0.0.0/0", "total"), "4294967296");
        assert_eq!(
            field("::/0", "total"),
            "340282366920938463463374607431768211456"
        );
        assert_eq!(field("1.2.3.4/32", "total"), "1");
    }

    /// RFC 3021: in a /31 both addresses are assignable, so the usual "minus two"
    /// convention must not apply.
    #[test]
    fn point_to_point_and_single_host_blocks_use_every_address() {
        assert_eq!(field("10.0.0.0/31", "usable"), "2");
        assert_eq!(field("10.0.0.0/31", "first_host"), "10.0.0.0");
        assert_eq!(field("10.0.0.0/31", "last_host"), "10.0.0.1");
        assert_eq!(field("10.0.0.7/32", "usable"), "1");
        assert_eq!(field("10.0.0.7/32", "first_host"), "10.0.0.7");
    }

    /// A bare address is its own single-host block rather than a parse error.
    #[test]
    fn a_missing_prefix_defaults_to_a_single_host() {
        assert_eq!(field("8.8.8.8", "network"), "8.8.8.8/32");
        assert_eq!(field("2001:db8::1", "network"), "2001:db8::1/128");
    }

    #[test]
    fn ipv6_reports_a_range_and_no_broadcast() {
        assert_eq!(field("2001:db8::1/48", "version"), "IPv6");
        assert_eq!(field("2001:db8::1/48", "network"), "2001:db8::/48");
        assert_eq!(
            field("2001:db8::1/48", "last_host"),
            "2001:db8:0:ffff:ffff:ffff:ffff:ffff"
        );
        assert!(field("2001:db8::1/48", "broadcast").starts_with("n/a"));
        // No network/broadcast carve-out in IPv6: every address in the block is usable.
        assert_eq!(field("2001:db8::/126", "usable"), "4");
    }

    #[test]
    fn scope_classifies_the_well_known_blocks() {
        let cases = [
            ("10.1.2.3/8", "private"),
            ("192.168.0.1/16", "private"),
            ("172.16.5.5/12", "private"),
            ("127.0.0.1", "loopback"),
            ("169.254.1.1/16", "link-local"),
            ("100.64.0.1/10", "shared (CGNAT)"),
            ("224.0.0.1/4", "multicast"),
            ("8.8.8.8", "public"),
            ("::1", "loopback"),
            ("fd00::1/8", "unique local"),
            ("fe80::1/10", "link-local"),
            ("2001:db8::/32", "documentation"),
            ("2606:4700::/32", "global"),
        ];
        for (cidr, want) in cases {
            assert_eq!(field(cidr, "scope"), want, "{cidr}");
        }
    }

    #[test]
    fn bad_input_names_the_field() {
        for bad in ["", "not an ip", "192.168.1.1/33", "::1/129", "10.0.0.1/x"] {
            let err = run(bad).unwrap_err();
            assert!(
                matches!(err, ToolError::InvalidInput { field: "cidr", .. }),
                "{bad:?}: {err:?}"
            );
        }
    }
}
