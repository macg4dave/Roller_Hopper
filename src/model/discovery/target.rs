//! What the user types as the network to scan: one IPv4 subnet, such as `192.168.1.0/24`, or
//! one address. Lists, ranges and exclusions are not supported.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

/// The widest subnet a run may scan: a /16, 65,536 addresses.
const WIDEST: u8 = 16;

/// Why typed text is not a network to scan, for the UI to word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetError {
    Empty,
    /// Neither an IPv4 address nor an IPv4 subnet.
    Invalid,
    /// A subnet wider than a /16.
    TooWide,
}

/// The subnet `text` names: a subnet with its prefix length, whose host bits are dropped, or a
/// lone address as a /32.
pub(crate) fn parse_target(text: &str) -> Result<Ipv4Net, TargetError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(TargetError::Empty);
    }
    let subnet = if text.contains('/') {
        text.parse::<Ipv4Net>().map_err(|_| TargetError::Invalid)?
    } else {
        let address = text.parse::<Ipv4Addr>().map_err(|_| TargetError::Invalid)?;
        Ipv4Net::from(address)
    };
    if subnet.prefix_len() < WIDEST {
        return Err(TargetError::TooWide);
    }
    Ok(subnet.trunc())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(text: &str) -> Ipv4Net {
        text.parse().unwrap()
    }

    #[test]
    fn a_subnet_or_an_address_is_a_target() {
        assert_eq!(
            parse_target(" 192.168.20.0/24 "),
            Ok(net("192.168.20.0/24"))
        );
        assert_eq!(
            parse_target("192.168.20.105/24"),
            Ok(net("192.168.20.0/24")),
            "this computer's own address and prefix"
        );
        assert_eq!(parse_target("10.0.0.7"), Ok(net("10.0.0.7/32")));
        assert_eq!(parse_target("10.0.0.0/16"), Ok(net("10.0.0.0/16")));
        assert_eq!(parse_target("10.0.0.6/31"), Ok(net("10.0.0.6/31")));
    }

    #[test]
    fn anything_else_says_why_not() {
        assert_eq!(parse_target("  "), Err(TargetError::Empty));
        for bad in [
            "192.168.20",
            "192.168.20.0/33",
            "my network",
            "fd00::/64",
            "10.0.0.0/",
        ] {
            assert_eq!(parse_target(bad), Err(TargetError::Invalid), "{bad}");
        }
        assert_eq!(parse_target("10.0.0.0/8"), Err(TargetError::TooWide));
    }
}
