//! Network Mapper: a local network mapper. The map starts empty; the discovery page scans this
//! computer's subnets for real, and devices can be added by hand.

mod ai;
mod ai_settings;
mod files;
#[cfg(test)]
mod mock_data;
mod model;
mod project;
mod scan;
mod ui;
mod ui_bridge;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

use slint::ComponentHandle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = ui::AppWindow::new()?;
    ui_bridge::select_language(&ui);
    let state = if std::env::args_os().any(|argument| argument == "--mock-data") {
        let project = project::mock().map_err(|error| {
            std::io::Error::other(format!(
                "the built-in mock project could not be opened: {error:?}"
            ))
        })?;
        ui_bridge::AppState::with_project(project, None)
    } else {
        ui_bridge::AppState::new(model::Network::default())
    };
    let state = Rc::new(RefCell::new(state));
    // One AI worker, started with the AI settings window and shared with Import with AI.
    let (ai_events, ai_inbox) = mpsc::channel();
    let worker = ai_settings::connect(&ui, ai_events)?;
    ui_bridge::connect(&ui, &state, worker, ai_inbox);
    Ok(ui.run()?)
}
