# Discovery and scanning types

Roller Hopper currently discovers an IPv4 subnet without credentials, elevation, packet capture,
external programs or a separate service. It combines a neighbour-table and ping sweep with
reverse DNS, TCP and UDP probes, DNS-SD/mDNS announcements, SSDP replies and bounded local UPnP
device descriptions. The declared mDNS and UPnP capabilities are retained as evidence for the
deterministic classifier and Import with AI; they are not treated as trusted instructions.

SSDP description URLs are fetched only when they use plain HTTP and name the replying IPv4
address literally. Response counts, XML size and network timeouts are bounded. This deliberately
rejects redirects, hostnames and off-device URLs so a network announcement cannot turn the app
into a general-purpose URL fetcher.

## Good candidates for later

| Type | What it could add | Advantages | Costs and risks | Suggested use |
| --- | --- | --- | --- | --- |
| IPP printer attributes | Printer make/model, supported document formats, colour/duplex capability and state | Standard, detailed and often available without authentication; much stronger than port 631 alone | Attribute parsing and response limits are required; queries can wake or log activity on a printer | Query only hosts advertising `_ipp`/`_ipps` or exposing an IPP port |
| ONVIF WS-Discovery and device information | Camera/NVR identity, scopes, profiles and media endpoints | Strong camera evidence and useful model/manufacturer data | SOAP/XML complexity, multicast duplication and uneven vendor implementations; detailed calls often require credentials | Discover without credentials, then keep authenticated enrichment explicitly opt-in |
| WS-Discovery/WSD | Windows devices, printers and scanners with endpoint types and metadata | Complements mDNS on Windows-heavy networks and identifies WSD printers better than ports | SOAP/XML and IPv4/IPv6 multicast complexity; metadata retrieval may expose many transient endpoints | Add a bounded multicast browse and retain only host identity and declared types |
| SNMP | `sysDescr`, `sysObjectID`, interface inventory, bridge tables, LLDP neighbours and printer MIB data | The richest standard source for managed switches, routers, UPS units and printers | SNMPv1/v2 community strings are credentials and often disabled; SNMPv3 requires a substantial security/configuration UI; walking large trees can be slow | Add only as a user-configured, credential-aware enrichment step; never guess community strings |
| HTTP/HTTPS metadata | Page title, server product, redirects, favicon or known device API descriptors | Broad coverage and can distinguish web-managed appliances | Web content is hostile, highly variable and easy to misclassify; TLS validation and redirects need careful policy | Keep requests tiny and targeted to already-open web ports; prefer standard descriptors over page scraping |
| TLS certificates | Subject/SAN names, issuer and validity | Passive within a normal TLS handshake and sometimes reveals appliance identity | Self-signed certificates are common; names may be stale, private or shared; certificate data is not proof of device type | Record as low-strength identity evidence on known TLS ports |
| SSH host key and greeting | Stable host-key fingerprint plus implementation/version greeting | Helps recognise the same server after an address change and can provide OS evidence | Must not attempt authentication; banners can be spoofed and version collection has privacy implications | Extend the existing banner probe with a bounded key exchange only if stable identity matching is wanted |
| SMB negotiation | Windows/Samba dialect and server capabilities | Better Windows/NAS distinction than an open port alone | Protocol complexity and security-sensitive implementation history; names and versions can be hidden | Use a maintained in-process parser and stop before authentication or share enumeration |
| LLMNR, NBNS and NetBIOS node status | Windows and older appliance names, workgroup and adapter identity | Useful where DNS-SD is absent | Legacy broadcast protocols, spoofable answers and duplicate/conflicting names | Treat only as weak naming evidence and make conflicts visible |
| DHCP lease-table import | Authoritative lease name, address, MAC and lease state from the gateway | Excellent correlation and finds sleeping clients | There is no universal unauthenticated query; usually needs router credentials or a vendor API | Implement as explicit router integrations, never as a background credential probe |
| Vendor discovery protocols | Detailed identity for Ubiquiti, MikroTik, Synology, QNAP and similar ecosystems | Often provides exact model, firmware and role with one local broadcast | Each protocol adds vendor-specific parsing and maintenance; some expose sensitive configuration | Add only for well-documented protocols with bounded read-only requests and representative fixtures |

## Poor default fits

### LLDP packet capture

LLDP is link-layer traffic normally visible only on the local Ethernet segment. A workstation
behind a switch does not receive every other port's LLDP frames, and capturing even the frames it
can see needs raw-link access or a capture driver on common desktop platforms. Switch SNMP LLDP
tables are far more useful for topology, but require configured credentials. Therefore direct
LLDP capture does not fit the scanner's present no-driver, no-elevation design.

### Broad vulnerability or authentication scanning

Password attempts, default-community guessing, exploit checks, directory enumeration and large
web crawls are security scanners rather than capability discovery. They create lockout, audit,
privacy and consent problems and should not be added to an automatic home-network mapping pass.

### Full protocol interrogation of every open port

Running every possible application handshake against every host would make Full and Everything
scans much slower and noisier. Capability probes should be selected by strong hints such as an
advertisement or a known open port, have short independent deadlines, and report partial coverage
instead of making the entire scan fail when one device misbehaves.

## Evidence rules for future additions

- Preserve the protocol and exact declared fact separately from an interpretation.
- Bound packets, replies, redirects, response bodies, XML depth, text lengths and elapsed time.
- Treat all network text as untrusted and never execute or follow instructions from it.
- Prefer deterministic classifications for explicit declarations; use AI only for ambiguity.
- Do not replace values the user entered manually.
- Do not attempt credentials unless the user configured that protocol and initiated the query.
- Keep source gates, live-network checks and installed-app behaviour as separate evidence.
