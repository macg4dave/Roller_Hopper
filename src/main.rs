//! Network Mapper: a local network mapper. The map starts empty; the discovery page scans this
//! computer's subnets for real, and devices can be added by hand.

mod ai;
mod ai_settings;
#[cfg(test)]
mod mock_data;
mod model;
mod project;
mod scan;
mod ui;
mod ui_bridge;

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let ui = ui::AppWindow::new()?;
    ui_bridge::select_language(&ui);
    let network = model::Network::default();
    let state = Rc::new(RefCell::new(ui_bridge::AppState::new(network)));
    ui_bridge::connect(&ui, &state);
    ai_settings::connect(&ui)?;
    ui.run()
}
