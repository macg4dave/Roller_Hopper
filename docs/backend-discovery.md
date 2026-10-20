# Network discovery and scanning strategy

Status: proposed strategy; [questions](backend-decisions.md) still govern limits, permissions,
unattended activity. Windows, Linux and macOS, local and routed/VLAN targets, and broad port
scanning are confirmed requirements. IPv4 comes first; IPv6 is deferred. This review performed
no network operations.
All collection executes inside the app. External scanners, helper executables, localhost
backends and separately installed services/drivers are excluded. Selected devices and normal
network discovery/name services are data sources, not hosting for the application's backend.

The selected engine approach is [upstream netscli-core through Cargo](backend-scanner-engine.md),
without a fork or copied scanner. Methods below describe desired evidence and coverage, not
instructions to implement the probes ourselves. Stock-release Windows packaging and run
control remain qualification gaps. Any necessary platform-specific exception must be
explained and approved; no source integration is authorised in this documentation task.

## Plan each run explicitly

Enumerate local interfaces, addresses and routes before offering targets. Show which interface
and source address a run will use, distinguish VPN/virtual adapters, and require selected
targets rather than scanning every prefix on the map. A routed subnet can be scanned only
through an existing route; the application does not create routes, join arbitrary VLANs or
configure equipment. Keep stable user network contexts: a prefix or transient interface index
alone cannot distinguish two networks using `192.168.1.0/24`.

Preserve the confirmed parent-device/child-interface structure: each interface retains its
own addresses, services and relationships under its owning device. Reconcile endpoint
observations into that structure with identity evidence; several interfaces do not require
several duplicate parent devices. Ownership is distinct from an interface's network uplink.
This clarifies result storage, not automatic target selection or permission to probe every
subnet represented by a stored card.

Freeze the target set, transport/port range, methods, exclusions and limits at Start. Expand
targets lazily, reject accidental huge/invalid ranges, handle IPv4 /31 and /32 deliberately,
and avoid network/broadcast addresses where applicable. Binding source addresses and selecting
multicast interfaces are per-platform capabilities to verify; a default-route helper alone
does not prove that a requested destination uses the chosen adapter.

**Recommended separation within the existing scan workflow:** discovery, optional selected
service enrichment, and explicitly selected broad port coverage. Support the requested broad
scan without making all 65,535 ports on every host the default. TCP-connect probing can cover
the full selected TCP range; UDP needs protocol-aware queries and different outcome semantics.
The user selected TCP ranges plus named UDP discovery protocols (Q4a), rather than a blanket
UDP sweep. Exact range/exclusion rules remain planning details; workload bounds remain Q7.
Identification depth is user-selected up to what the adopted tools support (Q17), including
active identification where available. Keep broader identification available in the backend
even if the initial GUI exposes less. Unsupported methods and required platform exceptions
must be explicit; tool availability is not permission to configure or exploit devices.

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

For managed topology, a forwarding-database entry shows learned reachability through a bridge
port, not necessarily a direct cable. Retain switch, FDB/VLAN context, timestamp and entry state;
map bridge-port numbers through `dot1dBasePortIfIndex` before matching interface records.
An FDB ID is not automatically a VLAN ID, and port zero/aged entries cannot prove adjacency.
Keep this evidence distinct from LLDP neighbours and manual links.
[Q-BRIDGE MIB](https://datatracker.ietf.org/doc/html/rfc4363),
[Bridge MIB mapping](https://datatracker.ietf.org/doc/html/rfc4188)

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

On macOS 15+, local-network privacy also applies to ordinary sockets. Terminal-launched and
root processes receive different treatment from a normally launched app, so `cargo run` is
not packaged-GUI permission evidence. Future acceptance needs the signed app launched normally,
with fresh permission state, denial and subsequent grant; no such session was run here.
[Apple TN3179](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy)

For mdns-sd 0.21.6, successful construction is not socket readiness: lazy multicast failures
arrive through `monitor()`. Consume monitor and result events off the UI thread. A full browse
channel terminates the subscription; treat unexpected disconnect as lost/partial coverage,
not successful completion. Do not wait indefinitely for a `SearchStopped` event or retry
without bounds. [Daemon API](https://docs.rs/mdns-sd/latest/mdns_sd/struct.ServiceDaemon.html),
[release changes](https://raw.githubusercontent.com/keepsimple1/mdns-sd/v0.21.6/CHANGELOG.md)

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
their timeouts; final in-flight results may appear while paused. The user confirmed these
pause semantics in Q6a. Resume only starts remaining work. Stop cancels scheduling, signals
active operations, drains/abandons them safely, retains valid partial observations and reports
cancellation acknowledgement. The user confirmed that navigation keeps a run active, close
cancels it and restart never resumes traffic silently (Q6b). Shutdown wait limits remain Q6;
a token alone cannot interrupt every blocking/native call.

Measure elapsed time using a monotonic clock; persist wall-clock start/end separately.
Report per-stage completed/eligible work and coverage. A variable-length enrichment stage can
be indeterminate; do not fabricate a smooth percentage or call it finished at 100% scheduling.
Terminal delivery and save outcomes are distinct. Closing the app should stop new work, finish
a bounded save/shutdown and never silently continue scanning in a background service.
Apply all retained observation batches before acknowledging their terminal outcome; a run must
not show Completed while earlier accepted findings are still waiting to enter the document.

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

The agreed policy is **manual scans and refresh only** (Q5). Do not schedule checks or keep
network discovery running after the user-started run. Open, navigation and network changes
do not start traffic. Last-seen times and cached facts may age without active collection;
show last-known status rather than inventing fresh online/offline observations.
Reconcile each manual run against protected values using the proposed
[field warning](backend-architecture.md#field-precedence-and-topology). Cached facts or an AI
guess never count as a fresh successful probe.
