//! Connection lines as SVG path commands, drawn in the colour and line style of the group of the
//! device each line hangs from.

use super::convert;
use crate::model::{Connection, ConnectionKind, DeviceId, GroupColour, LineStyle, Network, Point};
use crate::ui;

/// How far apart the zigzag and wavy styles swing to each side, and how far off the route,
/// in map units.
const BEND_LENGTH: f32 = 5.0;
const BEND_DEPTH: f32 = 2.5;

/// How a set of lines is drawn alike: its parent's group colour and line style, known or
/// inferred, highlighted or not.
type Look = (Option<GroupColour>, LineStyle, ConnectionKind, bool);

/// SVG path commands for the `links`, each drawn along its route, and the `trunks`, one set per
/// look, highlighted sets last.
pub(super) fn lines(
    network: &Network,
    links: &[(Connection, [Point; 3])],
    trunks: &[(DeviceId, [Point; 2])],
    selected: Option<DeviceId>,
) -> Vec<ui::LineSet> {
    let is_selected = |id| selected == Some(id);
    // Every line hangs from a device: a link from its upstream device, a trunk from its own.
    let link_lines = links.iter().map(|&(link, route)| {
        let highlighted = is_selected(link.upstream) || is_selected(link.downstream);
        (link.upstream, link.kind, highlighted, route.to_vec())
    });
    let trunk_lines = trunks
        .iter()
        .map(|&(id, trunk)| (id, ConnectionKind::Known, is_selected(id), trunk.to_vec()));
    let mut sets: Vec<(Look, Vec<String>)> = Vec::new();
    for (parent, kind, highlighted, points) in link_lines.chain(trunk_lines) {
        let parent = network.device(parent);
        // A line takes the group colour of the device it hangs from, unless it is highlighted;
        // it keeps that group's style either way.
        let colour = parent
            .filter(|_| !highlighted)
            .and_then(|d| network.colour(d));
        let style = parent.map(|d| network.line_style(d)).unwrap_or_default();
        let path = path(style, &points);
        let look = (colour, style, kind, highlighted);
        match sets.iter_mut().find(|(l, _)| *l == look) {
            Some((_, paths)) => paths.push(path),
            None => sets.push((look, vec![path])),
        }
    }
    sets.sort_by_key(|((_, _, _, highlighted), _)| *highlighted);
    let sets = sets
        .into_iter()
        .map(|((group, style, kind, highlighted), paths)| ui::LineSet {
            commands: paths.join(" ").into(),
            colour: convert::colour(group),
            inferred: kind == ConnectionKind::Inferred,
            selected: highlighted,
            thick: style == LineStyle::Thick,
        });
    sets.collect()
}

/// A polyline through `points` in the given style.
fn path(style: LineStyle, points: &[Point]) -> String {
    match style {
        LineStyle::Solid | LineStyle::Thick => solid(points),
        LineStyle::Dashed => dashed(points, &[6.0, 4.0]),
        LineStyle::LongDashes => dashed(points, &[14.0, 6.0]),
        LineStyle::Dotted => dashed(points, &[1.5, 3.5]),
        LineStyle::DashDot => dashed(points, &[8.0, 3.0, 1.5, 3.0]),
        LineStyle::Zigzag => bent(points, true),
        LineStyle::Wavy => bent(points, false),
    }
}

/// A polyline as SVG path commands.
fn solid(points: &[Point]) -> String {
    let commands = points.iter().enumerate().map(|(i, p)| {
        let command = if i == 0 { 'M' } else { 'L' };
        format!("{command}{:.1} {:.1}", p.x, p.y)
    });
    commands.collect::<Vec<_>>().join(" ")
}

/// A polyline broken into dashes, each its own SVG sub-path. `pattern` lists dash and gap
/// lengths in turn, repeating; it starts afresh on each straight stretch.
fn dashed(points: &[Point], pattern: &[f32]) -> String {
    let mut dashes = Vec::new();
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let length = (b.x - a.x).hypot(b.y - a.y);
        let along = |distance: f32| {
            let t = distance / length;
            Point {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            }
        };
        let mut start = 0.0;
        for (i, &step) in pattern.iter().cycle().enumerate() {
            if start >= length {
                break;
            }
            if i % 2 == 0 {
                dashes.push(solid(&[along(start), along((start + step).min(length))]));
            }
            start += step;
        }
    }
    dashes.join(" ")
}

/// A polyline that swings from side to side of its route, in straight zigzags (`sharp`) or
/// smooth waves. Each straight stretch holds whole swings, so it meets the route at every corner.
fn bent(points: &[Point], sharp: bool) -> String {
    let mut commands = Vec::new();
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        if commands.is_empty() {
            commands.push(format!("M{:.1} {:.1}", a.x, a.y));
        }
        let length = (b.x - a.x).hypot(b.y - a.y);
        if length <= 0.0 {
            continue;
        }
        let (dx, dy) = ((b.x - a.x) / length, (b.y - a.y) / length);
        // A point `distance` along the stretch and `offset` to its side.
        let at = |distance: f32, offset: f32| Point {
            x: a.x + dx * distance - dy * offset,
            y: a.y + dy * distance + dx * offset,
        };
        let swings = (length / BEND_LENGTH).round().max(1.0);
        let step = length / swings;
        let (mut swing, mut side) = (1.0, 1.0);
        while swing <= swings {
            let back = at(swing * step, 0.0);
            let middle = (swing - 0.5) * step;
            commands.push(if sharp {
                let peak = at(middle, side * BEND_DEPTH);
                format!("L{:.1} {:.1} L{:.1} {:.1}", peak.x, peak.y, back.x, back.y)
            } else {
                // A quadratic curve reaches half way to its control point.
                let control = at(middle, side * 2.0 * BEND_DEPTH);
                format!(
                    "Q{:.1} {:.1} {:.1} {:.1}",
                    control.x, control.y, back.x, back.y
                )
            });
            swing += 1.0;
            side = -side;
        }
    }
    commands.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Positions, Role};

    const LINE: [Point; 2] = [Point { x: 0.0, y: 0.0 }, Point { x: 20.0, y: 0.0 }];

    #[test]
    fn dashed_line_breaks_into_dashes_that_stay_on_the_line() {
        let line = [Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 25.0 }];
        let expected = "M0.0 0.0 L0.0 6.0 M0.0 10.0 L0.0 16.0 M0.0 20.0 L0.0 25.0";
        assert_eq!(dashed(&line, &[6.0, 4.0]), expected);
        let dash_dot = "M0.0 0.0 L0.0 8.0 M0.0 11.0 L0.0 12.5 M0.0 15.5 L0.0 23.5";
        assert_eq!(dashed(&line, &[8.0, 3.0, 1.5, 3.0]), dash_dot);
    }

    #[test]
    fn a_zigzag_swings_to_both_sides_and_ends_on_the_route() {
        let zigzag = bent(&LINE, true);
        assert!(zigzag.starts_with("M0.0 0.0 L2.5 2.5 L5.0 0.0 L7.5 -2.5 L10.0 0.0"));
        assert!(zigzag.ends_with("L20.0 0.0"), "{zigzag}");
    }

    #[test]
    fn a_wave_is_made_of_curves_that_end_on_the_route() {
        let wave = bent(&LINE, false);
        assert!(wave.starts_with("M0.0 0.0 Q2.5 5.0 5.0 0.0 Q7.5 -5.0 10.0 0.0"));
        assert_eq!(wave.matches('Q').count(), 4);
        assert!(wave.ends_with("20.0 0.0"), "{wave}");
    }

    #[test]
    fn every_style_draws_something_from_the_start_of_the_route() {
        for style in LineStyle::ALL {
            assert!(path(style, &LINE).starts_with("M0.0 0.0"), "{style:?}");
        }
    }

    /// Whether some line set has the given look.
    fn has(sets: &[ui::LineSet], colour: ui::GroupColour, inferred: bool, selected: bool) -> bool {
        sets.iter()
            .any(|s| (s.colour, s.inferred, s.selected) == (colour, inferred, selected))
    }

    /// Every link of the sample network, routed as laid out.
    fn routed(network: &Network) -> (Positions, Vec<(Connection, [Point; 3])>) {
        let arrangement = crate::model::Arrangement::default();
        let positions = crate::model::tree_layout(network, &arrangement);
        let inner = arrangement.inner_neighbours(network, None);
        let links = network.connections().iter().map(|&c| {
            let route = crate::model::link_route(&positions, &inner, c.upstream, c.downstream);
            (c, route.unwrap())
        });
        let links = links.collect();
        (positions, links)
    }

    #[test]
    fn selection_highlights_its_links_and_draws_them_last() {
        let network = crate::mock_data::network();
        let (_, links) = routed(&network);
        let plain = lines(&network, &links, &[], None);
        assert!(plain.iter().all(|s| !s.selected));

        let access_point = DeviceId(3);
        let highlighted = lines(&network, &links, &[], Some(access_point));
        let none = ui::GroupColour::None;
        assert!(has(&highlighted, none, false, true), "uplink to the router");
        assert!(has(&highlighted, none, true, true), "wireless clients");
        let first_highlighted = highlighted.iter().position(|s| s.selected);
        let last_plain = highlighted.iter().rposition(|s| !s.selected);
        assert!(last_plain < first_highlighted);

        // A device with none behind it lights up only its own stretch of line.
        let pc = lines(&network, &links, &[], Some(DeviceId(5)));
        let lit = pc.iter().filter(|s| s.selected);
        let paths: usize = lit.map(|s| s.commands.matches('M').count()).sum();
        assert_eq!(paths, 1);
    }

    #[test]
    fn lines_take_the_colour_and_style_of_the_device_they_hang_from() {
        let mut network = crate::mock_data::network();
        let (positions, links) = routed(&network);
        // A colour of its own, so other groups' lines cannot be mistaken for the router's.
        let group = network.add_group("Gateway").unwrap();
        network.set_group_colour(group, Some(GroupColour::Red));
        network.set_group_line_style(group, LineStyle::Thick);
        network.set_role_group(&Role::Router, Some(group));

        let router = DeviceId(1);
        let trunks = [router, DeviceId(2)]
            .map(|id| (id, crate::model::trunk(&network, &positions, id).unwrap()));
        let sets = lines(&network, &links, &trunks, None);
        let red: Vec<_> = sets
            .iter()
            .filter(|s| s.colour == ui::GroupColour::Red)
            .collect();
        assert!(red.iter().all(|s| s.thick), "the group's style");
        let known = red.iter().find(|s| !s.inferred);
        let router_links = network
            .connections()
            .iter()
            .filter(|c| c.upstream == router);
        let expected = router_links
            .filter(|c| c.kind == ConnectionKind::Known)
            .count()
            + 1;
        assert_eq!(
            known.map(|s| s.commands.matches('M').count()),
            Some(expected),
            "the router's links and trunk, and only those"
        );
    }
}
