//! Network Mapper: a UI prototype of a local network mapper using mock data only.

mod ai;
mod ai_settings;
mod mock_data;
mod model;
mod ui;
mod ui_bridge;

use std::cell::RefCell;
use std::rc::Rc;

use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let ui = ui::AppWindow::new()?;
    ui_bridge::select_language(&ui);
    let state = Rc::new(RefCell::new(ui_bridge::AppState::new(mock_data::network())));
    ui_bridge::connect(&ui, &state);
    ai_settings::connect(&ui)?;
    ui.run()
}
