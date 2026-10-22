//! Pure network data and logic. Knows nothing about Slint, so a real backend can feed it later.

mod arrangement;
mod card;
mod device;
mod discovery;
mod groups;
mod layout;
mod network;
mod os;
mod roles;
mod services;
mod snap;

pub(crate) use arrangement::{Arrangement, Level};
pub(crate) use card::{CardId, MacAddr, NetworkCard};
pub(crate) use device::{
    Device, DeviceId, DeviceKind, Guest, GuestKind, Icon, LastSeen, Role, Status,
};
pub(crate) use discovery::{
    AiImportPhase, Answer, Decision, Discovery, Finding, Found, FoundFilter, FoundSort, LogEntry,
    LogEvent, Method, Parent, Phase, ScanMode, ScanOptions, ScanProgress, TargetError,
};
pub(crate) use groups::{GroupColour, GroupId, LineStyle};
pub(crate) use layout::{Point, Positions, link_route, tree_layout, trunk};
#[cfg(test)]
pub(crate) use network::CardLink;
pub(crate) use network::{Connection, ConnectionKind, Network, StatusFilter};
pub(crate) use os::{OsClue, OsFamily, OsHint};
pub(crate) use services::{ServiceProtocol, Services};
pub(crate) use snap::{Snap, snap_target};
