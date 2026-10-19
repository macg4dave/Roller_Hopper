//! Pure network data and logic. Knows nothing about Slint, so a real backend can feed it later.

mod device;
mod layout;
mod network;
mod roles;

pub(crate) use device::{Device, DeviceId, DeviceKind, LastSeen, MacAddr, Role, Service, Status};
pub(crate) use layout::{Point, Positions, route, tree_layout};
pub(crate) use network::{Connection, ConnectionKind, Network, StatusFilter};
