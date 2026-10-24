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

use std::collections::HashSet;

/// Slint exposes model IDs as signed integers and reserves negative values for "none".
pub(super) const MAX_MODEL_ID: u32 = i32::MAX as u32;

/// Allocates `count` distinct IDs, preferring a monotonic run after the current maximum and
/// falling back to gaps. The fallback matters for valid projects whose highest ID is the UI
/// limit; such a project must remain editable instead of overflowing or producing `-1` IDs.
pub(super) fn fresh_ids(used: impl Iterator<Item = u32>, count: usize) -> Option<Vec<u32>> {
    if count == 0 {
        return Some(Vec::new());
    }
    let used: HashSet<u32> = used.collect();
    let after_max = used.iter().copied().max().unwrap_or(0).checked_add(1);
    if let Some(first) = after_max {
        let additional = u32::try_from(count).ok()?.checked_sub(1)?;
        if first
            .checked_add(additional)
            .is_some_and(|last| last <= MAX_MODEL_ID)
        {
            return Some((first..).take(count).collect());
        }
    }
    let ids: Vec<u32> = (1..=MAX_MODEL_ID)
        .filter(|id| !used.contains(id))
        .take(count)
        .collect();
    (ids.len() == count).then_some(ids)
}

pub(crate) use arrangement::{Arrangement, Level};
pub(crate) use card::{CardId, MacAddr, NetworkCard};
pub(crate) use device::{Device, DeviceId, DeviceKind, Icon, LastSeen, Role, Status};
pub(crate) use discovery::{
    AiImportPhase, Answer, Decision, Discovery, Finding, Found, FoundFilter, FoundSort, LogEntry,
    LogEvent, Method, Parent, Phase, ScanMode, ScanOptions, ScanProgress, TargetError,
};
pub(crate) use groups::{GroupColour, GroupId, GroupName, LineStyle, StandardGroup};
#[cfg(test)]
pub(crate) use layout::trunk;
pub(crate) use layout::{Point, Positions, link_route, tree_layout, trunks};
pub(crate) use network::{
    AutoMapConfidence, AutoMapProposal, AutoMapReason, Connection, ConnectionKind, Network,
};
pub(crate) use os::{OsClue, OsFamily, OsHint};
pub(crate) use services::{ServiceProtocol, ServiceSource, Services};
pub(crate) use snap::{Snap, SnapKind, snap_target};
