//! Network Mapper: a UI prototype of a local network mapper using mock data only.

mod ui;

use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    ui::AppWindow::new()?.run()
}
