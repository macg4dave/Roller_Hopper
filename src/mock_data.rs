//! The sample home network, and the fake refresh the Scan button runs on it.

use std::net::Ipv4Addr;

use crate::model::{
    Connection, ConnectionKind, Device, DeviceId, DeviceKind, GroupColour, Icon, Interface,
    LastSeen, LineStyle, MacAddr, Network, Role, Services, Status,
};

/// The home network's /24, and that of the separate lab network behind the Lab Router.
const HOME: [u8; 3] = [192, 168, 1];
const LAB: [u8; 3] = [10, 0, 0];

/// The device whose status flips on each simulated scan, so a refresh visibly changes something.
const FLAKY_DEVICE: DeviceId = DeviceId(13);

/// Minutes added to "last seen" of devices that do not answer a simulated scan.
const SCAN_INTERVAL_MINUTES: u32 = 30;

/// The custom role the sample virtual machines share.
const VIRTUAL_MACHINE: &str = "Virtual machine";
/// The icon the virtual machine role was given, from the icon folder.
const VIRTUAL_MACHINE_ICON: &str = "Computers/Virtual machine";

struct Spec {
    id: u32,
    name: &'static str,
    kind: DeviceKind,
    role: Role,
    status: Status,
    subnet: [u8; 3],
    host: u8,
    mac: [u8; 6],
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
            ip: address(self.subnet, self.host),
            mac: MacAddr(self.mac),
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

fn address([a, b, c]: [u8; 3], host: u8) -> Ipv4Addr {
    Ipv4Addr::new(a, b, c, host)
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
        mac,
        manufacturer: None,
        model: None,
        hostname: None,
        minutes_ago: 0,
        services: &[],
        notes: "",
    }
}

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
            hostname: Some("docker"),
            services: &[(22, "SSH"), (9443, "Portainer")],
            ..spec(
                19,
                "Docker Host",
                K::Server,
                23,
                [0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x03],
            )
        },
        Spec {
            manufacturer: Some("Proxmox"),
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
        // The lab network: seen only through the Docker Host's second interface.
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

fn connections() -> Vec<Connection> {
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
        (6, 17, Known),
        (6, 18, Known),
        (6, 19, Known),
        (6, 20, Known),
        (6, 21, Known),
        (22, 23, Known),
    ]
    .into_iter()
    .map(|(up, down, kind)| Connection {
        upstream: DeviceId(up),
        downstream: DeviceId(down),
        kind,
    })
    .collect()
}

/// The Docker Host's second virtual network card, on the lab network.
fn interfaces() -> Vec<Interface> {
    vec![Interface {
        device: DeviceId(19),
        upstream: DeviceId(22),
        ip: address(LAB, 23),
        mac: MacAddr([0xBC, 0x24, 0x11, 0x4A, 0x0E, 0x13]),
        kind: ConnectionKind::Known,
    }]
}

pub(crate) fn network() -> Network {
    let mut network = Network::new(devices(), connections(), interfaces());
    // Custom roles as if the user had added them: no standard role has the TV or console icon,
    // and a virtual machine is told apart from the host it runs on.
    let custom: [(&[u32], _, _); 3] = [
        (&[10], "TV", DeviceKind::Tv.icon()),
        (&[11], "Games console", DeviceKind::Console.icon()),
        (
            &[17, 18, 19, 20, 21],
            VIRTUAL_MACHINE,
            Icon::new(VIRTUAL_MACHINE_ICON),
        ),
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

    #[test]
    fn ids_ips_and_macs_are_unique() {
        let network = network();
        let devices = network.devices();
        let interfaces = network.interfaces();
        let addresses = devices.len() + interfaces.len();
        let unique =
            |values: Vec<String>, count| values.iter().collect::<HashSet<_>>().len() == count;
        assert!(unique(
            devices.iter().map(|d| format!("{:?}", d.id)).collect(),
            devices.len()
        ));
        let ips = devices
            .iter()
            .map(|d| d.ip)
            .chain(interfaces.iter().map(|i| i.ip));
        assert!(unique(ips.map(|ip| ip.to_string()).collect(), addresses));
        let macs = devices
            .iter()
            .map(|d| d.mac)
            .chain(interfaces.iter().map(|i| i.mac));
        assert!(unique(macs.map(|mac| mac.to_string()).collect(), addresses));
    }

    #[test]
    fn an_interface_joins_its_device_to_another_subnet() {
        let network = network();
        assert!(!network.interfaces().is_empty());
        let subnet = |ip: Ipv4Addr| ip.octets()[..3].to_vec();
        for interface in network.interfaces() {
            let device = network.device(interface.device).unwrap();
            let upstream = network.device(interface.upstream).unwrap();
            assert_ne!(subnet(interface.ip), subnet(device.ip), "{interface:?}");
            assert_eq!(subnet(interface.ip), subnet(upstream.ip), "{interface:?}");
            assert!(!network.is_within(interface.upstream, interface.device));
        }
    }

    #[test]
    fn connections_reference_existing_devices_and_form_trees() {
        let network = network();
        for c in network.connections() {
            assert!(network.device(c.upstream).is_some(), "{c:?}");
            assert!(network.device(c.downstream).is_some(), "{c:?}");
        }
        let downstream: HashSet<_> = network.connections().iter().map(|c| c.downstream).collect();
        assert_eq!(
            downstream.len(),
            network.connections().len(),
            "one uplink per device"
        );
        assert_eq!(
            network.devices().len() - downstream.len(),
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
    fn virtual_machines_share_a_role_and_hang_off_their_host() {
        let network = network();
        let role = Role::Custom(VIRTUAL_MACHINE.into());
        let icon = network
            .role_icon(&role)
            .map(|i| format!("icons/{}.svg", i.0));
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            icon.is_some_and(|file| folder.join(file).is_file()),
            "their icon ships with the app"
        );
        let vms: Vec<_> = network
            .devices()
            .iter()
            .filter(|d| d.role == role)
            .collect();
        assert_eq!(vms.len(), 5);
        for vm in vms {
            let uplink = network.connections().iter().find(|c| c.downstream == vm.id);
            assert_eq!(uplink.map(|c| c.upstream), Some(DeviceId(6)), "{}", vm.name);
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
