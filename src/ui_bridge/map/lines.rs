//! Connection lines as SVG path commands, drawn in the colour and line style of the group of the
//! device whose card each line hangs from.

use crate::model::{
    CardId, Connection, ConnectionKind, Device, GroupColour, LineStyle, Network, Point, Positions,
};
use crate::ui;
use crate::ui_bridge::convert;

/// How far apart the zigzag and wavy styles swing to each side, and how far off the route,
/// in map units.
const BEND_LENGTH: f32 = 5.0;
const BEND_DEPTH: f32 = 2.5;
/// How far a tie stays clear of the icon centres at its ends, just outside the circle drawn
/// round a selected icon, in map units.
const ICON_CLEARANCE: f32 = 28.0;
/// The short line each style is previewed on in the style dropdown; its box in the UI is
/// 48 × 12 map units.
const SAMPLE: [Point; 2] = [Point { x: 0.0, y: 6.0 }, Point { x: 48.0, y: 6.0 }];

/// How a set of lines is drawn alike: its parent's group colour and line style, known or
/// inferred, highlighted or not.
type Look = (Option<GroupColour>, LineStyle, ConnectionKind, bool);

/// SVG path commands for the `links`, each drawn along its route, the `trunks`, and the dotted
/// `ties` between the cards of a selected device, one set per look, highlighted sets last.
/// Lines to and from the `lit` cards are highlighted, and so are the ties.
pub(super) fn lines(
    network: &Network,
    links: &[(Connection, [Point; 3])],
    trunks: &[(CardId, [Point; 2])],
    lit: &[CardId],
    ties: &[[Point; 2]],
) -> Vec<ui::LineSet> {
    let is_lit = |card: CardId| lit.contains(&card);
    // Every line hangs from a card: a link from its upstream card, a trunk from its own.
    let link_lines = links.iter().map(|(link, route)| {
        let highlighted = is_lit(link.upstream) || is_lit(link.downstream);
        (link.upstream, link.kind, highlighted, route.to_vec())
    });
    let trunk_lines = trunks
        .iter()
        .map(|&(id, trunk)| (id, ConnectionKind::Known, is_lit(id), trunk.to_vec()));
    let mut sets: Vec<(Look, Vec<String>)> = Vec::new();
    let mut add = |look: Look, path: String| match sets.iter_mut().find(|(l, _)| *l == look) {
        Some((_, paths)) => paths.push(path),
        None => sets.push((look, vec![path])),
    };
    for (parent, kind, highlighted, points) in link_lines.chain(trunk_lines) {
        let parent = network.card_owner(parent);
        // A line takes the group colour of the device it hangs from, unless it is highlighted;
        // it keeps that group's style either way.
        let colour = parent
            .filter(|_| !highlighted)
            .and_then(|d| network.colour(d));
        let style = parent.map(|d| network.line_style(d)).unwrap_or_default();
        add((colour, style, kind, highlighted), path(style, &points));
    }
    for tie in ties {
        let look = (None, LineStyle::Dotted, ConnectionKind::Known, true);
        add(look, path(LineStyle::Dotted, tie));
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

/// The ties between the cards of `device` as `shown`, each to the next, so they read as one
/// device while it is selected. Each stays clear of the icons at its ends.
pub(super) fn ties(device: &Device, shown: &Positions) -> Vec<[Point; 2]> {
    let spots: Vec<Point> = device
        .cards
        .iter()
        .filter_map(|c| shown.get(&c.id).copied())
        .collect();
    let pairs = spots.windows(2).filter_map(|pair| match pair {
        [a, b] => clear_of_icons(*a, *b),
        _ => None,
    });
    pairs.collect()
}

/// The straight line from `a` to `b`, less its ends under the icons there; `None` when the icons
/// are so close that nothing is left.
fn clear_of_icons(a: Point, b: Point) -> Option<[Point; 2]> {
    let length = (b.x - a.x).hypot(b.y - a.y);
    if length <= 2.0 * ICON_CLEARANCE {
        return None;
    }
    let along = |t: f32| Point {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    };
    let clearance = ICON_CLEARANCE / length;
    Some([along(clearance), along(1.0 - clearance)])
}

/// A preview of every line style, in the order the style dropdown lists them.
pub(super) fn samples() -> Vec<ui::LineSet> {
    let sample = |style| ui::LineSet {
        commands: path(style, &SAMPLE).into(),
        thick: style == LineStyle::Thick,
        ..ui::LineSet::default()
    };
    LineStyle::ALL.into_iter().map(sample).collect()
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

    #[test]
    fn every_style_has_its_own_preview_in_dropdown_order() {
        let samples = samples();
        assert_eq!(samples.len(), LineStyle::ALL.len());
        let unique: std::collections::HashSet<_> = samples
            .iter()
            .map(|s| (s.commands.to_string(), s.thick))
            .collect();
        assert_eq!(unique.len(), samples.len(), "each looks different");
        let thick = samples.iter().position(|s| s.thick);
        assert_eq!(
            thick.and_then(|i| LineStyle::ALL.get(i)),
            Some(&LineStyle::Thick)
        );
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
        let links = network.connections().into_iter().map(|c| {
            let route = crate::model::link_route(&positions, &inner, c.upstream, c.downstream);
            (c, route.unwrap())
        });
        let links = links.collect();
        (positions, links)
    }

    #[test]
    fn lit_cards_highlight_their_links_and_draw_them_last() {
        let network = crate::mock_data::network();
        let (_, links) = routed(&network);
        let plain = lines(&network, &links, &[], &[], &[]);
        assert!(plain.iter().all(|s| !s.selected));

        let highlighted = lines(&network, &links, &[], &[CardId(3)], &[]);
        let none = ui::GroupColour::None;
        assert!(has(&highlighted, none, false, true), "uplink to the router");
        assert!(has(&highlighted, none, true, true), "wireless clients");
        let first_highlighted = highlighted.iter().position(|s| s.selected);
        let last_plain = highlighted.iter().rposition(|s| !s.selected);
        assert!(last_plain < first_highlighted);

        // A card with none behind it lights up only its own stretch of line.
        let pc = lines(&network, &links, &[], &[CardId(5)], &[]);
        assert_eq!(lit_paths(&pc), 1);
    }

    #[test]
    fn a_selected_devices_cards_are_tied_together_clear_of_their_icons() {
        let network = crate::mock_data::network();
        let (positions, links) = routed(&network);
        let router = network.device(crate::model::DeviceId(1)).unwrap();
        let ties = ties(router, &positions);
        assert_eq!(ties.len(), 1, "LAN to WAN");
        let [start, _] = ties[0];
        let lan = positions[&CardId(1)];
        assert!((start.x - lan.x).hypot(start.y - lan.y) >= ICON_CLEARANCE - 0.01);
        let with_ties = lines(&network, &links, &[], &[CardId(1), CardId(101)], &ties);
        let without = lines(&network, &links, &[], &[CardId(1), CardId(101)], &[]);
        assert!(lit_paths(&with_ties) > lit_paths(&without));
        assert_eq!(
            clear_of_icons(Point::default(), Point { x: 50.0, y: 0.0 }),
            None
        );
    }

    /// How many lines are highlighted.
    fn lit_paths(sets: &[ui::LineSet]) -> usize {
        let lit = sets.iter().filter(|s| s.selected);
        lit.map(|s| s.commands.matches('M').count()).sum()
    }

    #[test]
    fn lines_take_the_colour_and_style_of_the_device_they_hang_from() {
        let mut network = crate::mock_data::network();
        let (positions, links) = routed(&network);
        // A colour of its own, so other groups' lines cannot be mistaken for the router's.
        let others: Vec<_> = network.groups().iter().map(|(id, _)| id).collect();
        for other in others {
            network.set_group_colour(other, None);
        }
        let group = network.add_group("Gateway").unwrap();
        network.set_group_colour(group, Some(GroupColour::Red));
        network.set_group_line_style(group, LineStyle::Thick);
        network.set_role_group(&Role::Router, Some(group));

        let trunks = [CardId(1), CardId(2)]
            .map(|id| (id, crate::model::trunk(&network, &positions, id).unwrap()));
        let sets = lines(&network, &links, &trunks, &[], &[]);
        let red: Vec<_> = sets
            .iter()
            .filter(|s| s.colour == ui::GroupColour::Red)
            .collect();
        assert!(red.iter().all(|s| s.thick), "the group's style");
        let known = red.iter().find(|s| !s.inferred);
        // Every router's links, as the role is in the group, but only the trunk passed in.
        let is_router = |id| {
            network
                .card_owner(id)
                .is_some_and(|d| d.role == Role::Router)
        };
        let router_links = network
            .connections()
            .into_iter()
            .filter(|c| is_router(c.upstream));
        let expected = router_links
            .filter(|c| c.kind == ConnectionKind::Known)
            .count()
            + 1;
        assert_eq!(
            known.map(|s| s.commands.matches('M').count()),
            Some(expected),
            "the routers' links and trunk, and only those"
        );
    }
}
