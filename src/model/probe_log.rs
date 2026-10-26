//! What each protocol a scan asked said about a device, kept as the protocol gave it for the
//! debug view: the evidence behind what the scan concluded. Never saved to a project.

/// A protocol a scan asks devices, beyond the sweep and the port probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeProtocol {
    Ssdp,
}

impl ProbeProtocol {
    /// The protocol's own name, the same in every language.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ssdp => "SSDP",
        }
    }
}

/// One reply: the raw packet, or `field: value` lines in the protocol's own field names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProbeRecord {
    pub(crate) protocol: ProbeProtocol,
    /// The device's port that answered.
    pub(crate) port: u16,
    pub(crate) text: String,
}
