//! Pure network data and logic. Knows nothing about Slint, so a real backend can feed it later.

mod arrangement;
mod card;
mod device;
mod discovery;
mod groups;
mod layout;
mod network;
mod roles;
mod services;
mod snap;

pub(crate) use arrangement::{Arrangement, Level};
pub(crate) use card::{CardId, MacAddr, NetworkCard};
pub(crate) use device::{
    Device, DeviceId, DeviceKind, Guest, GuestKind, Icon, LastSeen, Role, Status,
};
pub(crate) use discovery::{
    Answer, Decision, Discovery, Finding, Found, FoundFilter, LogEntry, LogEvent, Method, Parent,
    Phase,
};
pub(crate) use groups::{GroupColour, GroupId, LineStyle};
pub(crate) use layout::{Point, Positions, link_route, tree_layout, trunk};
pub(crate) use network::{CardLink, Connection, ConnectionKind, Network, StatusFilter};
pub(crate) use services::Services;
pub(crate) use snap::{Snap, snap_target};
