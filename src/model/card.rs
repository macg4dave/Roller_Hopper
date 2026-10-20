//! A device's network cards: each has its own address on a subnet, and links on its own.

use std::fmt;

use ipnet::Ipv4Net;

/// Stable identifier of a network card, unique across the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CardId(pub(crate) u32);

/// A MAC address, shown in the usual colon-separated upper-case form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MacAddr(pub(crate) [u8; 6]);

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let octets: Vec<String> = self.0.iter().map(|octet| format!("{octet:02X}")).collect();
        f.write_str(&octets.join(":"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct NetworkCard {
    pub(crate) id: CardId,
    /// The card's address and the prefix length of its subnet.
    pub(crate) address: Ipv4Net,
    /// Detected with the card; `None` for a card the user added.
    pub(crate) mac: Option<MacAddr>,
}

impl NetworkCard {
    /// Case-insensitive match on the address or MAC address; `query` is already lower case.
    pub(crate) fn matches(&self, query: &str) -> bool {
        self.address.addr().to_string().contains(query)
            || self
                .mac
                .is_some_and(|mac| mac.to_string().to_lowercase().contains(query))
    }
}
