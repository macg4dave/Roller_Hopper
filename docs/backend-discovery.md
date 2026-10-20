# Network discovery and scanning strategy

Status: proposed strategy; [questions](backend-decisions.md) still govern limits, permissions,
unattended activity. Windows, Linux and macOS, local and routed/VLAN targets, and broad port
scanning are confirmed requirements. IPv4 comes first; IPv6 is deferred. This review performed
no network operations.
All collection executes inside the app. External scanners, helper executables, localhost
backends and separately installed services/drivers are excluded. Selected devices and normal
network discovery/name services are data sources, not hosting for the application's backend.

## Plan each run explicitly

Enumerate local interfaces, addresses and routes before offering targets. Show which interface
and source address a run will use, distinguish VPN/virtual adapters, and require selected
targets rather than scanning every prefix on the map. A routed subnet can be scanned only
through an existing route; the application does not create routes, join arbitrary VLANs or
configure equipment. Keep stable user network contexts: a prefix or transient interface index
alone cannot distinguish two networks using `192.168.1.0/24`.

Freeze the target set, transport/port range, methods, exclusions and limits at Start. Expand
targets lazily, reject accidental huge/invalid ranges, handle IPv4 /31 and /32 deliberately,
and avoid network/broadcast addresses where applicable. Binding source addresses and selecting
multicast interfaces are per-platform capabilities to verify; a default-route helper alone
does not prove that a requested destination uses the chosen adapter.

**Recommended separation within the existing scan workflow:** discovery, optional selected
service enrichment, and explicitly selected broad port coverage. Support the requested broad
scan without making all 65,535 ports on every host the default. TCP-connect probing can cover
the full selected TCP range; UDP needs protocol-aware queries and different outcome semantics.
Whether broad coverage includes UDP is Q4, not an assumption.

## Combine sources without overstating them

| Method | Useful evidence | Boundary and proposed use |
| --- | --- | --- |
| Local adapter/route metadata | Scanner's addresses, link state, gateway candidates | Bootstrap targets, not a remote-device inventory; select all relevant adapters explicitly |
| OS neighbour cache, ARP/NDP | Scoped on-link IP↔MAC hints, state | Cache may be stale/incomplete; cannot supply remote routed-host MACs. Re-read after permitted probes; do not label cached entries online automatically |
| ICMP echo | Positive response and approximate latency | Optional capability; no reply may be filtering/sleep/loss. Never make successful ping a prerequisite for TCP/service discovery |
| TCP connect | Open port, refusal, timeout/unreachable outcome | Works without raw packet construction; bound time/rate. Refusal differs from silent timeout; neither identifies device class |
| Reverse DNS | PTR name candidates | Use selected network's resolver policy and TTLs; not an authenticated device identity, and not proof of presence |
| mDNS/DNS-SD | Advertised names, services, addresses, TTL/TXT hints | Link-scoped and advertisement-dependent; not all devices announce themselves. Browse known useful types and service-type enumeration; one query does not discover every host |
| SSDP/UPnP | Device UUID/type and description hints | Optional device-class enrichment. Descriptions/URLs are untrusted; no automatic remote control or port mapping |
| SNMP and managed-network APIs | Interface identifiers, inventory, forwarding/LLDP tables, VLAN-aware evidence | Optional credentials and read-only requests. Prefer SNMPv3 authenticated encryption; no brute-force/default-community guessing |
| LLDP | Neighbour/chassis/port relationships | Link-neighbour information, not a magic subnet scan. Remote topology generally needs switch access; raw capture adds privileges/driver burden |
| ONVIF | Camera discovery and declared device details | Optional camera-specific enrichment; use existing protocol client, read-only operations and explicit credentials if needed |
| Vendor/OUI lookup | Manufacturer assignment hint | Offline dataset with provenance/date and distribution review; NIC vendor is not a device type or reliable physical-device manufacturer |

Protocol grounding: [Windows neighbour-table API](https://learn.microsoft.com/en-us/windows/win32/api/netioapi/nf-netioapi-getipnettable2),
[IPv6 neighbour discovery](https://www.rfc-editor.org/rfc/rfc4861.html),
[mDNS](https://www.rfc-editor.org/rfc/rfc6762.html),
[DNS-SD](https://www.rfc-editor.org/rfc/rfc6763.html),
[LLDP implementation documentation](https://github.com/lldpd/lldpd), and
[Bridge MIB](https://www.rfc-editor.org/rfc/rfc4188.html).

Collectors emit independent observations, not complete fabricated `Device`s. Validate scope,
addresses, lengths and identifiers, then reconcile the same endpoint's evidence across
methods. Preserve disagreements and method-specific failure. Do not invent calibrated
percentages or count correlated DNS/mDNS copies as independent confirmation. Distinguish
source certainty, uncalibrated model scores and identity confidence. Classify fields with
reasons; keep Unknown valid. AI consumes this evidence under
[the AI policy](backend-opportunities.md).

## Cross-platform capability boundary

| Capability | Windows | Linux | macOS |
| --- | --- | --- | --- |
| Interface metadata | Candidate: netdev native backend | Candidate: netdev native backend | Candidate: netdev native backend; some extra metadata can block |
| TCP/UDP ordinary sockets | Portable async APIs; firewall/routing may constrain | Same; namespace/capability context matters | Same; local-network access and firewall policy matter |
| Neighbour cache | IP Helper API semantics | Netlink-backed read candidate | Routing/socket or vetted library candidate |
| ICMP | Native ICMP API candidate; raw sockets are a separate permission issue | Datagram ping may be permitted by system policy, otherwise raw capability needed | Socket permissions and supported strategy need live verification |
| Multicast discovery | Interface binding, port sharing and firewall need acceptance | Interface selection and namespace boundaries need acceptance | Local-network permission, multicast joining and interface changes need acceptance |
| Packet capture/raw ARP/LLDP | Paths requiring Npcap/WinPcap installation are excluded | Native OS capture may require privileges; not first-release baseline | Native capture privileges require separate consideration; no helper programs |

This is an implementation-research matrix, not a claim that these paths have been tested.
[netdev](https://github.com/shellrow/netdev) advertises the three desktop platforms.
[libpnet](https://github.com/libpnet/libpnet) documents Windows capture-library requirements,
which rule out that external-driver path under the confirmed self-contained-app constraint;
the native [Windows ICMP API](https://learn.microsoft.com/en-us/windows/win32/api/icmpapi/nf-icmpapi-icmpsendecho2)
has different mechanics from raw ICMP sockets. Ordinary discovery should remain usable with
unavailable optional capabilities, reporting exactly what was skipped.

The project denies unsafe code. Native `windows`/`windows-sys` bindings do **not** automatically
provide a safe application-level neighbour/ICMP wrapper. Prefer a maintained safe dependency;
the current neighbour-wrapper shortlist is not yet qualified. Do not weaken lints or conceal
bespoke FFI in a new crate. This is a real selection gap to resolve before promising ARP/MAC
parity. Shelling out to `arp`, `ping`, Nmap or any other program is excluded.

For the deferred IPv6 phase, use known addresses from neighbours, DNS, service advertisements
and explicit targets; never enumerate a /64. Preserve link-local zones and multiple addresses.
The first release must state its IPv4 coverage rather than imply a complete dual-stack inventory.

## Lifecycle and failures

Proposed states: preparing, running, pausing/paused, cancelling, then completed, cancelled or
failed. The terminal result also records partial coverage and per-method issues; a completed
run with a failed optional collector must say so. Preparation failures send no probes.

Pause should stop launching new work while allowing in-flight operations to finish under
their timeouts; final in-flight results may appear while paused. Resume only starts remaining
work. Stop cancels scheduling, signals active operations, drains/abandons them safely, retains
valid partial observations and reports cancellation acknowledgement. These semantics need Q6
agreement; a token alone cannot interrupt every blocking/native call.

Measure elapsed time using a monotonic clock; persist wall-clock start/end separately.
Report per-stage completed/eligible work and coverage. A variable-length enrichment stage can
be indeterminate; do not fabricate a smooth percentage or call it finished at 100% scheduling.
Terminal delivery and save outcomes are distinct. Closing the app should stop new work, finish
a bounded save/shutdown and never silently continue scanning in a background service.

Missing ICMP, exhausted sockets, DNS failure, revoked permissions, disconnected adapters and
collector overload are recoverable method/run outcomes, not reasons to erase past inventory.
Avoid retry storms: bounded retries with backoff only where meaningful, and no automatic
expansion of targets or elevation. Full-scope coverage cannot be claimed after interruption.

## Safety and scale by design

Treat every hostname, TXT value, banner, XML body and identifier as untrusted data. Cap lengths,
parser depth, redirect count and time; escape log controls and render text as data. Device
description URLs can point outside the chosen network or to local services: constrain schemes,
resolved destinations and every redirect to approved targets, with no external entity fetching.
Never execute returned text or let AI turn it into commands.

Keep results local by default, redact credentials from errors, and do not require an online
vendor lookup or cloud inference. Broad scans require visible target/port scope, exclusions,
rate limits and an explicit run. No credential guessing, exploit checks, NSE script execution
or device configuration is implied by the mapper requirement.

Provisional resource budgets must be chosen for the required scale in Q7: cap simultaneous
probes and starts/second separately, bound per-device enrichment and total run work, keep only
a bounded UI log, and apply retention. Subnet size × port count, not host count alone, drives
scan cost. Validate on all three OSes before claiming capability parity.
