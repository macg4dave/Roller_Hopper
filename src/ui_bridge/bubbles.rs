//! Bubble interactions extend the existing groups, arrangement and guarded map undo.

use std::cell::RefCell;
use std::rc::Rc;

use slint::{ComponentHandle, SharedString};

use super::{AppState, convert, handler, view::sync};
use crate::model::{Bubble, GroupId, Point, valid_position};
use crate::ui::{AppWindow, Bridge, Labels};

#[cfg(test)]
mod tests;

impl AppState {
    fn bubble_group(&self, index: i32) -> Option<GroupId> {
        self.network
            .groups()
            .iter()
            .nth(convert::index(index))
            .map(|(id, _)| id)
    }

    fn edit_bubbles(&mut self, edit: impl FnOnce(&mut Self)) {
        self.finish_bubble_gesture();
        self.cancel_drag();
        let network = self.network.clone();
        let arrangement = self.arrangement.clone();
        edit(self);
        self.lay_out_at_rest();
        self.remember_map_edit(network, arrangement);
    }

    fn add_bubble(&mut self, name: &str, centre: Point) {
        let size = Bubble::INITIAL_SIZE;
        let origin = Point {
            x: centre.x - size.x / 2.0,
            y: centre.y - size.y / 2.0,
        };
        if !valid_position(origin)
            || !valid_position(Point {
                x: origin.x + size.x,
                y: origin.y + size.y,
            })
        {
            return;
        }
        self.edit_bubbles(|st| {
            if let Some(group) = st.network.add_group(name) {
                st.arrangement.add_bubble(group, origin);
            }
        });
    }

    fn set_bubble_member(&mut self, index: i32, join: bool) {
        let (Some(group), Some(device)) = (self.bubble_group(index), self.selected_device()) else {
            return;
        };
        if !join
            && !self
                .arrangement
                .bubbles()
                .iter()
                .any(|b| b.group == group && b.members.contains(&device))
        {
            return;
        }
        self.edit_bubbles(|st| {
            st.arrangement.set_bubble_member(
                &st.network,
                &mut st.positions,
                device,
                join.then_some(group),
            );
        });
    }

    fn change_bubble_geometry(&mut self, index: i32, at: Point, resize: bool) {
        let Some(group) = self.bubble_group(index) else {
            return;
        };
        if self.bubble_gesture.is_none() {
            self.cancel_drag();
            self.bubble_gesture = Some(self.arrangement.clone());
        }
        if resize {
            self.arrangement
                .resize_bubble(&self.network, &self.positions, group, at);
        } else {
            self.arrangement
                .move_bubble(&self.network, &mut self.positions, group, at);
        }
        self.lay_out_at_rest();
    }

    fn finish_bubble_gesture(&mut self) {
        if let Some(arrangement) = self.bubble_gesture.take() {
            self.remember_map_edit(self.network.clone(), arrangement);
        }
    }

    pub(in crate::ui_bridge) fn cancel_bubble_gesture(&mut self) {
        if let Some(arrangement) = self.bubble_gesture.take() {
            self.arrangement = arrangement;
            self.lay_out_at_rest();
        }
    }
}

pub(super) fn connect(ui: &AppWindow, state: &Rc<RefCell<AppState>>) {
    let bridge = ui.global::<Bridge<'_>>();
    bridge.set_bubble_default_width(Bubble::INITIAL_SIZE.x);
    bridge.set_bubble_default_height(Bubble::INITIAL_SIZE.y);
    let add = handler(ui, state, |st, (name, x, y): (SharedString, f32, f32)| {
        st.add_bubble(&name, Point { x, y });
    });
    bridge.on_add_bubble(move |name, x, y| add((name, x, y)));
    let member = handler(ui, state, |st, (index, join)| {
        st.set_bubble_member(index, join);
    });
    bridge.on_set_bubble_member(move |index, join| member((index, join)));
    let geometry = handler(ui, state, |st, (index, x, y, resize)| {
        st.change_bubble_geometry(index, Point { x, y }, resize);
    });
    bridge.on_change_bubble_geometry(move |index, x, y, resize| geometry((index, x, y, resize)));
    let finish = handler(ui, state, |st, ()| st.finish_bubble_gesture());
    bridge.on_finish_bubble_gesture(move || finish(()));
    let cancel = handler(ui, state, |st, ()| st.cancel_bubble_gesture());
    bridge.on_cancel_bubble_gesture(move || cancel(()));
    bridge.on_remove_bubble(handler(ui, state, |st, index| {
        if let Some(group) = st.bubble_group(index) {
            st.edit_bubbles(|st| st.arrangement.remove_bubble(group));
        }
    }));
    let rename = handler(ui, state, |st, (index, name): (i32, SharedString)| {
        if let Some(group) = st.bubble_group(index) {
            st.edit_bubbles(|st| st.network.rename_group(group, &name));
        }
    });
    bridge.on_rename_bubble(move |index, name| rename((index, name)));
}

pub(super) fn push(bridge: &Bridge<'_>, labels: &Labels<'_>, state: &AppState) {
    let selected = state.selected_device();
    let groups: Vec<_> = state.network.groups().iter().collect();
    let views = state
        .arrangement
        .bubbles()
        .iter()
        .filter_map(|bubble| {
            let (index, (_, name)) = groups
                .iter()
                .enumerate()
                .find(|(_, (id, _))| *id == bubble.group)?;
            Some(crate::ui::BubbleView {
                group_index: i32::try_from(index).ok()?,
                name: convert::group_label(labels, name),
                colour: convert::colour(state.network.groups().colour(bubble.group)),
                x: bubble.origin.x,
                y: bubble.origin.y,
                width: bubble.size.x,
                height: bubble.size.y,
                can_join: selected.is_some_and(|id| !bubble.members.contains(&id)),
                can_leave: selected.is_some_and(|id| bubble.members.contains(&id)),
            })
        })
        .collect();
    if let Some(model) = sync(&bridge.get_bubbles(), views) {
        bridge.set_bubbles(model);
    }
}
