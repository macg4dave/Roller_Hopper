//! Editing the network from the edit page, the details panel's edit mode and the discovery
//! details (for a found host on the map): the selected
//! device, its network cards and their open ports, moving cards to other parents, the role list,
//! the group list and the Layer 2 segments. Each area handles its own `Bridge` callbacks.

mod cards;
mod device;
mod groups;
mod moves;
mod ports;
mod roles;
mod segments;

pub(super) use moves::CardMove;

use std::cell::RefCell;
use std::rc::Rc;

use super::AppState;
use crate::ui::AppWindow;

/// Wires every editing callback to the state.
pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    device::connect(ui, state);
    cards::connect(ui, state);
    ports::connect(ui, state);
    roles::connect(ui, state);
    groups::connect(ui, state);
    segments::connect(ui, state);
    moves::connect(ui, state);
}
