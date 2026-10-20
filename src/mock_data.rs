//! The sample home network, and the fake refresh the Scan button runs on it.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;

#[cfg(test)]
use crate::model::Node;
use crate::model::{
    CardId, CardLink, ConnectionKind, Device, DeviceId, DeviceKind, GroupColour, Guest, GuestKind,
    Icon, LastSeen, LineStyle, MacAddr, Network, NetworkCard, Role, Services, Status,
};

/// The home network's /24, that of the separate lab network behind the Lab Router, the
/// Docker Host's /16 bridge network for its containers, and the router's side of the internet
/// connection (a documentation range, RFC 5737).
const HOME: [u8; 3] = [192, 168, 1];
const LAB: [u8; 3] = [10, 0, 0];
const DOCKER: [u8; 3] = [172, 17, 0];
const WAN: [u8; 3] = [203, 0, 113];

/// The device whose status flips on each simulated scan, so a refresh visibly changes something.
const FLAKY_DEVICE: DeviceId = DeviceId(13);

/// Minutes added to "last seen" of devices that do not answer a simulated scan.
const SCAN_INTERVAL_MINUTES: u32 = 30;

/// The custom role the sample virtual machines share.
const VIRTUAL_MACHINE: &str = "Virtual machine";
/// The icon the virtual machine role was given, from the icon folder.
const VIRTUAL_MACHINE_ICON: &str = "Computers/Virtual machine";
/// The custom role the sample containers share, and its icon.
const CONTAINER: &str = "Container";
const CONTAINER_ICON: &str = "Computers/Container";

/// A sample device. Its first network card has the same id as the device, so the links below
/// read as links between devices (or the router's first card).
struct Spec {
    id: u32,
    name: &'static str,
    kind: DeviceKind,
    role: Role,
    status: Status,
    subnet: [u8; 3],
    host: u8,
    prefix: u8,
    mac: [u8; 6],
    /// The id of the device it runs on, for a virtual machine or container.
    guest: Option<(u32, GuestKind)>,
    manufacturer: Option<&'static str>,
    model: Option<&'static str>,
    hostname: Option<&'static str>,
    minutes_ago: u32,
    services: &'static [(u16, &'static str)],
    notes: &'static str,
}

impl Spec {
    fn build(self) -> Device {
        let mut services = Services::default();
        for &(port, description) in self.services {
            services.add(port, description);
        }
        Device {
            id: DeviceId(self.id),
            name: self.name.to_owned(),
            kind: self.kind,
            role: self.role,
            icon: None,
            status: self.status,
            cards: card(self.id, self.subnet, self.host, self.prefix, self.mac)
                .into_iter()
                .collect(),
            guest: self.guest.map(|(host, kind)| Guest {
                host: DeviceId(host),
                kind,
            }),
            manufacturer: self.manufacturer,
            model: self.model,
            hostname: self.hostname,
            last_seen: LastSeen {
                minutes_ago: self.minutes_ago,
            },
            services,
            notes: self.notes.to_owned(),
        }
    }
}

/// A detected network card. `None` only for a prefix over 32, which the sample data never uses:
/// the tests check every sample device has its cards.
fn card(id: u32, [a, b, c]: [u8; 3], host: u8, prefix: u8, mac: [u8; 6]) -> Option<NetworkCard> {
    Some(NetworkCard {
        address: Some(Ipv4Net::new(Ipv4Addr::new(a, b, c, host), prefix).ok()?),
        mac: Some(MacAddr(mac)),
        ..NetworkCard::blank(CardId(id))
    })
}

/// Shorthand for the many devices that share most defaults, on the home network.
fn spec(id: u32, name: &'static str, kind: DeviceKind, host: u8, mac: [u8; 6]) -> Spec {
    Spec {
        id,
        name,
        kind,
        role: kind.default_role(),
        status: Status::Online,
        subnet: HOME,
        host,
        prefix: 24,
        mac,
        guest: None,
        manufacturer: None,
        model: None,
        hostname: None,
        minutes_ago: 0,
        services: &[],
        notes: "",
    }
}

const ON_LINUX_SERVER: Option<(u32, GuestKind)> = Some((6, GuestKind::VirtualMachine));
const ON_DOCKER_HOST: Option<(u32, GuestKind)> = Some((19, GuestKind::Container));

#[expect(
    clippy::too_many_lines,
    reason = "flat table of sample data; splitting it hides the network"
)]
fn devices() -> Vec<Device> {
    use DeviceKind as K;
    [
        Spec {
            manufacturer: Some("ASUS"),
            model: Some("RT-AX88U"),
            hostname: Some("router"),
            services: &[(53, "DNS"), (80, "HTTP"), (443, "HTTPS")],
            notes: "Main home router. ISP fibre comes in here.",
            ..spec(
                1,
                "Router",
                K::Router,
                1,
                [0x04, 0xD9, 0xF5, 0x3C, 0x4D, 0x5E],
            )
        },
        Spec {
            manufacturer: Some("Netgear"),
            model: Some("GS308E"),
            services: &[(80, "HTTP")],
            notes: "Under the desk in the study.",
            ..spec(
                2,
                "Main Switch",
                K::Switch,
                2,
                [0xA0, 0x40, 0xA0, 0x71, 0x22, 0x10],
            )
        },
        Spec {
            manufacturer: Some("TP-Link"),
            model: Some("EAP245"),
            hostname: Some("ap-living-room"),
            ..spec(
                3,
                "AP Living Room",
                K::AccessPoint,
                40,
                [0x50, 0xC7, 0xBF, 0x9A, 0x01, 0x44],
            )
        },
        Spec {
            manufacturer: Some("Synology"),
            model: Some("DS920+"),
            hostname: Some("nas"),
            services: &[(22, "SSH"), (445, "SMB"), (5000, "DSM")],
            notes: "Nightly backups at 02:00.",
            ..spec(4, "NAS", K::Nas, 10, [0x00, 0x11, 0x32, 0xA8, 0x5B, 0x07])
        },
        Spec {
            manufacturer: Some("Dell"),
            model: Some("OptiPlex 7090"),
            hostname: Some("desktop-7k2m"),
            services: &[(3389, "RDP")],
            ..spec(
                5,
                "Windows PC",
                K::Desktop,
                11,
                [0xD8, 0x9E, 0xF3, 0x12, 0x6A, 0xC1],
            )
        },
        Spec {
            manufacturer: Some("HP"),
            model: Some("ProDesk 600 G4"),
            hostname: Some("linux-server"),
            services: &[(22, "SSH"), (8006, "Proxmox")],
            notes: "Proxmox host for the virtual machines.",
            ..spec(
                6,
                "Linux Server",
                K::Server,
                20,
                [0x3C, 0x52, 0x82, 0x4E, 0x90, 0x2F],
            )
        },
        Spec {
            manufacturer: Some("HP"),
            model: Some("LaserJet Pro M404dn"),
            hostname: Some("hp-laserjet"),
            services: &[(80, "HTTP"), (631, "IPP"), (9100, "JetDirect")],
            ..spec(
                7,
                "HP Printer",
                K::Printer,
                30,
                [0x3C, 0x52, 0x82, 0x77, 0x13, 0xB4],
            )
        },
        Spec {
            manufacturer: Some("Apple"),
            model: Some("MacBook Pro 14\""),
            hostname: Some("macbook-pro"),
            ..spec(
                8,
                "MacBook Pro",
                K::Laptop,
                12,
                [0xF0, 0x18, 0x98, 0x5D, 0xE2, 0x31],
            )
        },
        Spec {
            manufacturer: Some("Apple"),
            model: Some("iPhone 15"),
            hostname: Some("iphone"),
            ..spec(
                9,
                "iPhone",
                K::Phone,
                60,
                [0xF0, 0x18, 0x98, 0x0B, 0x7C, 0x9A],
            )
        },
        Spec {
            manufacturer: Some("Samsung"),
            model: Some("QE55Q80C"),
            hostname: Some("samsung-tv"),
            services: &[(8001, "SmartView")],
            ..spec(
                10,
                "TV Living Room",
                K::Tv,
                50,
                [0x8C, 0x79, 0xF5, 0x1A, 0x2B, 0x3C],
            )
        },
        Spec {
            manufacturer: Some("Sony"),
            model: Some("PlayStation 5"),
            hostname: Some("ps5"),
            ..spec(
                11,
                "Game Console",
                K::Console,
                61,
                [0x00, 0xD9, 0xD1, 0x64, 0x88, 0x05],
            )
        },
        Spec {
            manufacturer: Some("Reolink"),
            model: Some("RLC-810A"),
            hostname: Some("camera-front"),
            services: &[(80, "HTTP"), (554, "RTSP")],
            notes: "Covers the drive and front door.",
            ..spec(
                12,
                "IP Camera Front",
                K::Camera,
                70,
                [0xEC, 0x71, 0xDB, 0x2E, 0x40, 0x9D],
            )
        },
        Spec {
            manufacturer: Some("TP-Link"),
            model: Some("Kasa KP115"),
            hostname: Some("kasa-plug"),
            ..spec(
                13,
                "Smart Plug",
                K::Smart,
                80,
                [0x1C, 0x3B, 0xF3, 0xC5, 0x17, 0x62],
            )
        },
        Spec {
            manufacturer: Some("Philips"),
            model: Some("Hue Bridge"),
            hostname: Some("philips-hue"),
            services: &[(80, "HTTP"), (443, "HTTPS")],
            ..spec(
                14,
                "Hue Bridge",
                K::Smart,
                81,
                [0x00, 0x17, 0x88, 0x6F, 0xA3, 0x18],
            )
        },
        Spec {
            manufacturer: Some("Lenovo"),
            model: Some("ThinkPad T14"),
            hostname: Some("work-laptop"),
            status: Status::Offline,
            minutes_ago: 2 * 24 * 60 + 190,
            notes: "Work laptop; only on the network on office days.",
            ..spec(
                15,
                "Work Laptop",
                K::Laptop,
                13,
                [0x54, 0xE1, 0xAD, 0x30, 0x8B, 0x46],
            )
        },
        Spec {
            status: Status::Unknown,
            minutes_ago: 45,
            ..spec(
                16,
                "Unknown Device",
                K::Unknown,
                90,
                [0x9A, 0x3F, 0x0C, 0x55, 0xD1, 0x08],
            )
        },
        // Virtual machines on the Linux Server; BC:24:11 is the Proxmox MAC prefix.
        Spec {
            manufacturer: Some("Proxmox"),
            guest: ON_LINUX_SERVER,
            hostname: Some("homeassistant"),
            services: &[(8123, "Home Assistant")],
            ..spec(
                17,
                "Home Assistant",
                K::Server,
                21,
                [0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x01],
            )
        },
        Spec {
            manufacturer: Some("Proxmox"),
            guest: ON_LINUX_SERVER,
            hostname: Some("pihole"),
            services: &[(53, "DNS"), (80, "HTTP")],
            notes: "Blocks adverts for the whole network.",
            ..spec(
                18,
                "Pi-hole",
                K::Server,
                22,
                [0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x02],
            )
        },
        Spec {
            manufacturer: Some("Proxmox"),
            guest: ON_LINUX_SERVER,
            hostname: Some("docker"),
            services: &[(22, "SSH")],
            ..spec(
                19,
                "Docker Host",
                K::Server,
                23,
                [0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x03],
            )
        },
        // Containers on the Docker Host's bridge network; 02:42 is Docker's MAC prefix.
        Spec {
            manufacturer: Some("Docker"),
            hostname: Some("portainer"),
            subnet: DOCKER,
            prefix: 16,
            guest: ON_DOCKER_HOST,
            services: &[(9443, "Portainer")],
            ..spec(
                24,
                "Portainer",
                K::Server,
                2,
                [0x02, 0x42, 0xAC, 0x11, 0x00, 0x02],
            )
        },
        Spec {
            manufacturer: Some("Docker"),
            hostname: Some("proxy"),
            subnet: DOCKER,
            prefix: 16,
            guest: ON_DOCKER_HOST,
            services: &[(80, "HTTP"), (81, "Admin"), (443, "HTTPS")],
            notes: "Reverse proxy for the home lab's web pages.",
            ..spec(
                25,
                "Nginx Proxy Manager",
                K::Server,
                3,
                [0x02, 0x42, 0xAC, 0x11, 0x00, 0x03],
            )
        },
        Spec {
            manufacturer: Some("Proxmox"),
            guest: ON_LINUX_SERVER,
            hostname: Some("win11-vm"),
            services: &[(3389, "RDP")],
            status: Status::Offline,
            minutes_ago: 5 * 60,
            notes: "Started only when needed.",
            ..spec(
                20,
                "Windows 11 VM",
                K::Desktop,
                24,
                [0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x04],
            )
        },
        Spec {
            manufacturer: Some("Proxmox"),
            guest: ON_LINUX_SERVER,
            hostname: Some("ubuntu-dev"),
            services: &[(22, "SSH")],
            ..spec(
                21,
                "Ubuntu Dev",
                K::Server,
                25,
                [0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x05],
            )
        },
        // The lab network: seen only through the Docker Host's second network card.
        Spec {
            manufacturer: Some("MikroTik"),
            model: Some("hAP ax²"),
            hostname: Some("lab-router"),
            subnet: LAB,
            services: &[(22, "SSH"), (80, "WebFig"), (8291, "Winbox")],
            notes: "Separate lab network for testing.",
            ..spec(
                22,
                "Lab Router",
                K::Router,
                1,
                [0x48, 0xA9, 0x8A, 0x21, 0x5C, 0x90],
            )
        },
        Spec {
            manufacturer: Some("Raspberry Pi"),
            model: Some("Raspberry Pi 5"),
            hostname: Some("lab-pi"),
            subnet: LAB,
            services: &[(22, "SSH")],
            ..spec(
                23,
                "Lab Pi",
                K::Server,
                50,
                [0xD8, 0x3A, 0xDD, 0x4C, 0x71, 0x0A],
            )
        },
    ]
    .into_iter()
    .map(Spec::build)
    .collect()
}

/// The Docker Host's second virtual network card, on the lab network.
const DOCKER_LAB_CARD: u32 = 100;
/// The router's second card, towards the internet.
const ROUTER_WAN_CARD: u32 = 101;

/// The sample devices, with the router's and the Docker Host's second cards, and names for the
/// cards of devices that have several.
fn devices_with_further_cards() -> Vec<Device> {
    let mut devices = devices();
    let wan_card = card(
        ROUTER_WAN_CARD,
        WAN,
        2,
        24,
        [0x04, 0xD9, 0xF5, 0x3C, 0x4D, 0x5F],
    )
    .map(|card| NetworkCard {
        name: "WAN".into(),
        gateway: Some(Ipv4Addr::new(203, 0, 113, 1)),
        ..card
    });
    let lab_card = card(
        DOCKER_LAB_CARD,
        LAB,
        23,
        24,
        [0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x13],
    )
    .map(|card| NetworkCard {
        name: "Lab".into(),
        ..card
    });
    for (id, first_name, further) in [(1, "LAN", wan_card), (19, "Home", lab_card)] {
        if let Some(device) = devices.iter_mut().find(|d| d.id == DeviceId(id)) {
            if let Some(first) = device.cards.first_mut() {
                first.name = first_name.into();
            }
            device.cards.extend(further);
        }
    }
    devices
}

/// Each card's link, as (upstream card, card, kind).
fn links() -> Vec<CardLink> {
    use ConnectionKind::{Inferred, Known};
    [
        (1, 4, Known),
        (1, 2, Known),
        (1, 3, Known),
        (2, 5, Known),
        (2, 6, Known),
        (2, 7, Known),
        (2, 11, Known),
        (2, 12, Known),
        (2, 14, Known),
        (2, 16, Inferred),
        (3, 8, Inferred),
        (3, 9, Inferred),
        (3, 10, Inferred),
        (3, 15, Inferred),
        (3, 13, Inferred),
        (22, 23, Known),
        (22, DOCKER_LAB_CARD, Known),
    ]
    .into_iter()
    .map(|(up, card, kind)| CardLink {
        upstream: CardId(up),
        card: CardId(card),
        kind,
    })
    .collect()
}

pub(crate) fn network() -> Network {
    let mut network = Network::new(devices_with_further_cards(), links());
    // Custom roles as if the user had added them: no standard role has the TV or console icon,
    // and virtual machines and containers are told apart from the hosts they run on.
    let custom: [(&[u32], _, _); 4] = [
        (&[10], "TV", DeviceKind::Tv.icon()),
        (&[11], "Games console", DeviceKind::Console.icon()),
        (
            &[17, 18, 19, 20, 21],
            VIRTUAL_MACHINE,
            Icon::new(VIRTUAL_MACHINE_ICON),
        ),
        (&[24, 25], CONTAINER, Icon::new(CONTAINER_ICON)),
    ];
    for (ids, name, icon) in custom {
        let Some(role) = network.add_custom_role(name, icon) else {
            continue;
        };
        for &id in ids {
            if let Some(device) = network.device_mut(DeviceId(id)) {
                device.role = role.clone();
            }
        }
    }
    add_groups(&mut network);
    network
}

/// Example groups, as if the user had made them. They follow how devices connect, so the lines
/// from the router, the switch and the access point each show in their own colour; Wi-Fi lines
/// wave.
fn add_groups(network: &mut Network) {
    let tv = Role::Custom("TV".into());
    let console = Role::Custom("Games console".into());
    let virtual_machine = Role::Custom(VIRTUAL_MACHINE.into());
    let container = Role::Custom(CONTAINER.into());
    let groups = [
        (
            "Network core",
            GroupColour::Purple,
            LineStyle::Solid,
            vec![Role::Router],
        ),
        (
            "Wired",
            GroupColour::Blue,
            LineStyle::Solid,
            vec![
                Role::Switch,
                Role::Desktop,
                Role::Server,
                Role::Nas,
                Role::Printer,
                virtual_machine,
                container,
            ],
        ),
        (
            "Wi-Fi",
            GroupColour::Green,
            LineStyle::Wavy,
            vec![Role::AccessPoint, Role::Laptop, Role::Phone],
        ),
        (
            "Smart home",
            GroupColour::Orange,
            LineStyle::Solid,
            vec![Role::Camera, Role::Iot, tv, console],
        ),
    ];
    for (name, colour, style, roles) in groups {
        let Some(group) = network.add_group(name) else {
            continue;
        };
        network.set_group_colour(group, Some(colour));
        network.set_group_line_style(group, style);
        for role in &roles {
            network.set_role_group(role, Some(group));
        }
    }
}

/// Pretends a scan ran: responsive devices are seen now, silent ones age, and one device flips
/// between online and offline. No network access takes place.
pub(crate) fn simulate_scan(network: &mut Network) {
    if let Some(flaky) = network.device_mut(FLAKY_DEVICE) {
        flaky.status = match flaky.status {
            Status::Online => Status::Offline,
            Status::Offline | Status::Unknown => Status::Online,
        };
    }
    for device in network.devices_mut() {
        device.last_seen.minutes_ago = match device.status {
            Status::Online => 0,
            Status::Offline | Status::Unknown => {
                device.last_seen.minutes_ago + SCAN_INTERVAL_MINUTES
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn all_unique<T: std::hash::Hash + Eq>(values: &[T]) -> bool {
        values.iter().collect::<HashSet<_>>().len() == values.len()
    }

    #[test]
    fn ids_ips_and_macs_are_unique_and_every_device_has_a_card() {
        let network = network();
        let devices = network.devices();
        assert!(devices.iter().all(|d| !d.cards.is_empty()));
        let ids: Vec<_> = devices.iter().map(|d| d.id).collect();
        assert!(all_unique(&ids));
        let cards: Vec<_> = devices.iter().flat_map(|d| &d.cards).collect();
        let card_ids: Vec<_> = cards.iter().map(|c| c.id).collect();
        assert!(all_unique(&card_ids));
        let ips: Option<Vec<_>> = cards.iter().map(|c| c.address.map(|a| a.addr())).collect();
        let ips = ips.expect("every sample card has an address");
        assert!(all_unique(&ips));
        let macs: Option<Vec<_>> = cards.iter().map(|c| c.mac).collect();
        let macs = macs.expect("every sample card has a MAC");
        assert!(all_unique(&macs));
    }

    #[test]
    fn a_further_card_joins_its_device_to_another_subnet() {
        let network = network();
        let ghost = Node::Card(CardId(DOCKER_LAB_CARD));
        let link = network.uplink(ghost).unwrap();
        let docker = network.node_device(ghost).unwrap();
        let upstream = network.node_device(link.upstream).unwrap();
        let [home, lab] = [0, 1].map(|at| docker.cards[at].address.unwrap());
        assert_ne!(home.network(), lab.network());
        let lab_router = upstream.primary_card().unwrap().address.unwrap();
        assert!(lab_router.contains(&lab.addr()), "{lab} on {lab_router}");
    }

    #[test]
    fn the_routers_cards_are_on_their_own_subnets_with_a_gateway_out() {
        let network = network();
        let router = network.device(DeviceId(1)).unwrap();
        let [lan, wan] = [0, 1].map(|at| &router.cards[at]);
        let wan_net = wan.address.unwrap();
        assert_ne!(lan.address.unwrap().network(), wan_net.network());
        assert!(wan.gateway.is_some_and(|g| wan_net.contains(&g)));
    }

    #[test]
    fn connections_reference_existing_cards_and_form_trees() {
        let network = network();
        let cards = network.devices().iter().flat_map(|d| &d.cards);
        for link in cards.filter_map(|card| network.link(card.id)) {
            assert!(network.card_owner(link.upstream).is_some(), "{link:?}");
        }
        assert_eq!(
            links().len(),
            network
                .devices()
                .iter()
                .flat_map(|d| &d.cards)
                .filter(|c| network.link(c.id).is_some())
                .count(),
            "every link is of a sample card"
        );
        let downstream: HashSet<_> = network.connections().iter().map(|c| c.downstream).collect();
        assert_eq!(
            downstream.len(),
            network.connections().len(),
            "one uplink per device"
        );
        assert_eq!(
            network.nodes().len() - downstream.len(),
            2,
            "the home network and the lab network"
        );
    }

    #[test]
    fn the_router_switch_and_access_point_each_have_their_own_colour() {
        let network = network();
        let colour = |id| network.device(DeviceId(id)).and_then(|d| network.colour(d));
        let parents = [colour(1), colour(2), colour(3)];
        assert!(parents.iter().all(Option::is_some), "{parents:?}");
        let distinct: HashSet<_> = parents.iter().collect();
        assert_eq!(distinct.len(), 3, "{parents:?}");
    }

    #[test]
    fn guests_share_a_role_and_run_on_their_host() {
        let network = network();
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for (name, kind, host, count) in [
            (VIRTUAL_MACHINE, GuestKind::VirtualMachine, 6, 5),
            (CONTAINER, GuestKind::Container, 19, 2),
        ] {
            let role = Role::Custom(name.into());
            let icon = network
                .role_icon(&role)
                .map(|i| format!("icons/{}.svg", i.0));
            assert!(
                icon.is_some_and(|file| folder.join(file).is_file()),
                "{name}: the icon ships with the app"
            );
            let guests: Vec<_> = network
                .devices()
                .iter()
                .filter(|d| d.role == role)
                .collect();
            assert_eq!(guests.len(), count, "{name}");
            for guest in guests {
                let on = Some(Guest {
                    host: DeviceId(host),
                    kind,
                });
                assert_eq!(guest.guest, on, "{}", guest.name);
            }
        }
    }

    #[test]
    fn scan_flips_the_flaky_device_and_ages_silent_ones() {
        let mut network = network();
        let status = |n: &Network, id| n.device(id).map(|d| (d.status, d.last_seen.minutes_ago));
        let before = status(&network, DeviceId(15));

        simulate_scan(&mut network);
        assert_eq!(status(&network, FLAKY_DEVICE), Some((Status::Offline, 30)));
        assert_eq!(
            status(&network, DeviceId(15)),
            before.map(|(s, m)| (s, m + 30))
        );

        simulate_scan(&mut network);
        assert_eq!(status(&network, FLAKY_DEVICE), Some((Status::Online, 0)));
    }
}
