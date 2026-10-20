//! Editing the network from the edit page and the details panel's edit mode: the selected
//! device, its network cards and their open ports, the role list and the group list. Each area
//! handles its own `Bridge` callbacks.

mod cards;
mod device;
mod groups;
mod ports;
mod roles;

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
}
