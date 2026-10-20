//! Pure network data and logic. Knows nothing about Slint, so a real backend can feed it later.

mod device;
mod groups;
mod layout;
mod network;
mod roles;
mod services;

pub(crate) use device::{Device, DeviceId, DeviceKind, Icon, LastSeen, MacAddr, Role, Status};
pub(crate) use groups::{GroupColour, GroupId};
pub(crate) use layout::{Point, Positions, Snap, route, snap_target, tree_layout};
pub(crate) use network::{Connection, ConnectionKind, Network, StatusFilter};
pub(crate) use services::Services;
