//! The AI settings window's sidebar: scans the network typed, as the discovery page does, and
//! lists the hosts found. Picking one puts the evidence Import with AI would send for it in the
//! device text, so the prompt can be tried on real hosts. The run is the sidebar's own, apart
//! from the discovery page's and the map.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString, Weak};

use crate::ai;
use crate::model::{Discovery, Network};
use crate::scan::{self, Scan};
use crate::ui::{AiScanHost, AiSettingsWindow};
use crate::ui_bridge::{Running, sync, target_error};

/// The sidebar's run, and the scan running for it.
#[derive(Default)]
struct Sidebar {
    discovery: RefCell<Discovery>,
    /// Nothing is on a map here, so every host found is new.
    network: Network,
    running: Rc<Running>,
}

/// The network box starts with this computer's own subnet, as the discovery page's does.
pub(super) fn connect(window: &AiSettingsWindow) {
    let sidebar = Rc::new(Sidebar::default());
    let own = scan::local_subnet()
        .map(|s| s.to_string())
        .unwrap_or_default();
    window.set_scan_target(own.as_str().into());
    sidebar.discovery.borrow_mut().target_text = own;
    show(window, &sidebar.discovery.borrow());

    let (typing, typed) = (window.as_weak(), Rc::clone(&sidebar));
    window.on_set_scan_target(move |text| {
        typed.discovery.borrow_mut().target_text = text.into();
        if let Some(open) = typing.upgrade() {
            show(&open, &typed.discovery.borrow());
        }
    });
    let (starting, start) = (window.as_weak(), Rc::clone(&sidebar));
    window.on_scan(move || begin(&starting, &start));
    let (stopping, stop) = (window.as_weak(), Rc::clone(&sidebar));
    window.on_stop_scan(move || {
        stop.running.end();
        stop.discovery.borrow_mut().stop();
        if let Some(open) = stopping.upgrade() {
            show(&open, &stop.discovery.borrow());
        }
    });
    let picking = window.as_weak();
    window.on_select_scan_host(move |index| {
        let Some(open) = picking.upgrade() else {
            return;
        };
        let discovery = sidebar.discovery.borrow();
        let found = usize::try_from(index)
            .ok()
            .and_then(|i| discovery.found().get(i));
        if let Some(found) = found {
            open.set_input(ai::evidence(&found.device, &found.methods).into());
            open.set_scan_host_index(index);
        }
    });
}

/// Starts a run of the network typed, unless it is not one or a run is busy, forgetting the
/// last run's hosts.
fn begin(window: &Weak<AiSettingsWindow>, sidebar: &Rc<Sidebar>) {
    let started = {
        let mut discovery = sidebar.discovery.borrow_mut();
        let Ok(target) = discovery.next_target() else {
            return;
        };
        if !discovery.start(target) {
            return;
        }
        match Scan::start(target, discovery.options) {
            Ok(scan) => Some(scan),
            Err(error) => {
                discovery.fail(error.to_string());
                None
            }
        }
    };
    if let Some(open) = window.upgrade() {
        open.set_scan_host_index(-1);
        show(&open, &sidebar.discovery.borrow());
    }
    let Some(scan) = started else { return };
    let (window, this) = (window.clone(), Rc::downgrade(sidebar));
    sidebar.running.watch(scan, move |scan| {
        let Some(open_sidebar) = this.upgrade() else {
            return false;
        };
        let mut discovery = open_sidebar.discovery.borrow_mut();
        scan::record(
            &mut discovery,
            scan.progress(),
            scan.events(),
            &open_sidebar.network,
        );
        if let Some(open) = window.upgrade() {
            show(&open, &discovery);
        }
        discovery.busy()
    });
}

/// Sends the run to the sidebar: whether the network typed can be scanned, how far a running
/// scan has got, and the hosts found in address order.
fn show(window: &AiSettingsWindow, discovery: &Discovery) {
    let int = |n: usize| i32::try_from(n).unwrap_or(i32::MAX);
    window.set_scan_target_error(target_error(discovery.next_target().err()));
    window.set_scanning(discovery.busy());
    let (swept, addresses) = discovery.progress();
    window.set_scan_swept(int(swept));
    window.set_scan_addresses(int(addresses));
    let hosts = discovery.shown().into_iter().filter_map(|(index, _)| {
        let found = discovery.found().get(index)?;
        Some(AiScanHost {
            index: int(index),
            name: SharedString::from(found.device.name.trim()),
            ip: found.ip.to_string().into(),
        })
    });
    if let Some(model) = sync(&window.get_scan_hosts(), hosts.collect()) {
        window.set_scan_hosts(model);
    }
}
