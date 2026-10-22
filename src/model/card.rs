//! A device's network cards: each is a child of its device, with its own IPv4 settings, open
//! ports and icon, and plugs into a card of another device on its own. Cards are what the map
//! shows.

use std::fmt;
use std::net::Ipv4Addr;
use std::str::FromStr;

use ipnet::Ipv4Net;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

use super::device::Icon;
use super::services::Services;

/// Stable identifier of a network card, unique across the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct CardId(pub(crate) u32);

/// A MAC address, shown in the usual colon-separated upper-case form, and saved in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MacAddr(pub(crate) [u8; 6]);

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let octets: Vec<String> = self.0.iter().map(|octet| format!("{octet:02X}")).collect();
        f.write_str(&octets.join(":"))
    }
}

impl Serialize for MacAddr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for MacAddr {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse()
            .map_err(|BadMac| de::Error::custom("not a MAC address"))
    }
}

/// Why a typed MAC address was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BadMac;

impl FromStr for MacAddr {
    type Err = BadMac;

    /// Six pairs of hex digits, separated by colons or dashes, in either case.
    fn from_str(text: &str) -> Result<Self, BadMac> {
        let mut octets = [0; 6];
        let mut parts = text.trim().split([':', '-']);
        for octet in &mut octets {
            let part = parts.next().filter(|p| p.len() == 2).ok_or(BadMac)?;
            *octet = u8::from_str_radix(part, 16).map_err(|_| BadMac)?;
        }
        match parts.next() {
            None => Ok(Self(octets)),
            Some(_) => Err(BadMac),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NetworkCard {
    pub(crate) id: CardId,
    /// The user's name for it, such as "LAN"; empty shows a numbered placeholder.
    pub(crate) name: String,
    /// The card's address and the prefix length of its subnet; `None` until the user gives one
    /// to a card they added.
    pub(crate) address: Option<Ipv4Net>,
    pub(crate) gateway: Option<Ipv4Addr>,
    /// Detected with the card, or typed by the user.
    pub(crate) mac: Option<MacAddr>,
    /// Who made the card, from the IEEE registry entry for its MAC address. Not the device's
    /// maker: a laptop's Wi-Fi card is often someone else's.
    pub(crate) vendor: Option<String>,
    /// The ports open at the card's address.
    pub(crate) services: Services,
    /// What kind of connection it is, shown in the lists and as a badge on its device's icon on
    /// the map: Ethernet, Wi-Fi, 5G and so on, or any icon the user picks.
    pub(crate) icon: Icon,
}

impl NetworkCard {
    /// The icon a card starts with, one of the icons that ship with the app.
    pub(crate) const DEFAULT_ICON: &str = "Network cards/Ethernet";

    /// A card the user added, with nothing filled in yet.
    pub(crate) fn blank(id: CardId) -> Self {
        Self {
            id,
            name: String::new(),
            address: None,
            gateway: None,
            mac: None,
            vendor: None,
            services: Services::default(),
            icon: Icon::new(Self::DEFAULT_ICON),
        }
    }

    /// Case-insensitive match on the address or MAC address; `query` is already lower case.
    pub(crate) fn matches(&self, query: &str) -> bool {
        self.address
            .is_some_and(|net| net.addr().to_string().contains(query))
            || self
                .mac
                .is_some_and(|mac| mac.to_string().to_lowercase().contains(query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mac_address_is_read_with_colons_or_dashes_in_either_case() {
        let mac = MacAddr([0x8c, 0x79, 0xf5, 0x1a, 0x2b, 0x3c]);
        assert_eq!(" 8c:79:F5:1a:2B:3c ".parse(), Ok(mac));
        assert_eq!("8C-79-F5-1A-2B-3C".parse(), Ok(mac));
        assert_eq!(mac.to_string().parse(), Ok(mac), "round trip");
        for bad in [
            "",
            "8C:79:F5:1A:2B",
            "8C:79:F5:1A:2B:3C:00",
            "8C:79:F5:1A:2B:3G",
            "8C79F51A2B3C",
        ] {
            assert_eq!(bad.parse::<MacAddr>(), Err(BadMac), "{bad}");
        }
    }

    #[test]
    fn a_blank_card_matches_no_search() {
        assert!(!NetworkCard::blank(CardId(1)).matches("1"));
    }
}
