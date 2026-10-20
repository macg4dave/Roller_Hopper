//! Pure network data and logic. Knows nothing about Slint, so a real backend can feed it later.

mod arrangement;
mod device;
mod groups;
mod layout;
mod network;
mod roles;
mod services;
mod snap;

pub(crate) use arrangement::{Arrangement, Level};
pub(crate) use device::{Device, DeviceId, DeviceKind, Icon, LastSeen, MacAddr, Role, Status};
pub(crate) use groups::{GroupColour, GroupId, LineStyle};
pub(crate) use layout::{Ghost, Point, Positions, ghosts, link_route, tree_layout, trunk};
pub(crate) use network::{Connection, ConnectionKind, Interface, Network, StatusFilter};
pub(crate) use services::Services;
pub(crate) use snap::{Snap, snap_target};
