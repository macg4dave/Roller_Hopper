//! The map's tree. Its nodes are the devices and the ghosts of their further network cards.
//! Each node hangs from the node of the card it is plugged into.
//!
//! A device with several cards hangs from one of them (its tree card). Each of its other cards
//! is drawn as a ghost of the device: the ghost sits where that card is plugged in (or under
//! its own device when it is plugged in nowhere) and heads whatever is plugged into the card.

use std::iter;

use super::{CardLink, ConnectionKind, Network};
use crate::model::card::CardId;
use crate::model::device::{Device, DeviceId};

/// Something drawn on the map: a device, or the ghost of one of its network cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Node {
    Device(DeviceId),
    Card(CardId),
}

/// A line of the map's tree, from a node towards the router (`upstream`) to one further away
/// (`downstream`). It runs through a card's link, from a guest's host to the guest, or from a
/// device to the ghost of a card plugged in nowhere. The last two are known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Connection {
    pub(crate) upstream: Node,
    pub(crate) downstream: Node,
    pub(crate) kind: ConnectionKind,
}

impl Network {
    /// The device a network card belongs to.
    pub(crate) fn card_owner(&self, card: CardId) -> Option<&Device> {
        let mut devices = self.devices.iter();
        devices.find(|d| d.cards.iter().any(|c| c.id == card))
    }

    /// The link of `card`, if it is plugged in.
    pub(crate) fn link(&self, card: CardId) -> Option<&CardLink> {
        self.links.iter().find(|l| l.card == card)
    }

    /// The card a device hangs from. For a guest, this is its first card, which reaches the
    /// network through its host. Otherwise it is its first card that is plugged in. `None` for
    /// a device at the top of a tree.
    pub(crate) fn tree_card(&self, device: &Device) -> Option<CardId> {
        if device.guest.is_some() {
            return device.primary_card().map(|c| c.id);
        }
        let mut cards = device.cards.iter().map(|c| c.id);
        cards.find(|&card| self.link(card).is_some())
    }

    /// Whether `card` is drawn as a ghost: every card of a device with several, apart from the
    /// one the device hangs from.
    pub(crate) fn has_ghost(&self, card: CardId) -> bool {
        let owner = self.card_owner(card);
        owner.is_some_and(|d| d.cards.len() > 1 && self.tree_card(d) != Some(card))
    }

    /// The node that stands for `card` on the map: its ghost, or else its device.
    pub(crate) fn node_of_card(&self, card: CardId) -> Option<Node> {
        let owner = self.card_owner(card)?;
        Some(if self.has_ghost(card) {
            Node::Card(card)
        } else {
            Node::Device(owner.id)
        })
    }

    /// The device a node is, or is a ghost of.
    pub(crate) fn node_device(&self, node: Node) -> Option<&Device> {
        match node {
            Node::Device(id) => self.device(id),
            Node::Card(card) => self.card_owner(card),
        }
    }

    /// Every node on the map, in device order, each device followed by its ghosts.
    pub(crate) fn nodes(&self) -> Vec<Node> {
        let nodes = self.devices.iter().flat_map(|d| {
            let ghosts = d.cards.iter().filter(|c| self.has_ghost(c.id));
            iter::once(Node::Device(d.id)).chain(ghosts.map(|c| Node::Card(c.id)))
        });
        nodes.collect()
    }

    /// The card a link into `node` plugs into. For a ghost, that is its card. For a device, it
    /// is its only card, or the card it hangs from when it has several. `None` for a device with
    /// several cards at the top of a tree: something plugs into one of its ghosts instead.
    pub(crate) fn socket(&self, node: Node) -> Option<CardId> {
        match node {
            Node::Card(card) => Some(card),
            Node::Device(id) => match self.device(id)?.cards.as_slice() {
                [only] => Some(only.id),
                _ => self.tree_card(self.device(id)?),
            },
        }
    }

    /// The card whose link `node` hangs by. This is `None` for a guest, which hangs from its
    /// host, and for a node that is plugged in nowhere.
    fn hanging_card(&self, node: Node) -> Option<CardId> {
        let card = match node {
            Node::Device(id) => {
                let device = self.device(id).filter(|d| d.guest.is_none())?;
                self.tree_card(device)?
            }
            Node::Card(card) => card,
        };
        self.link(card).map(|_| card)
    }

    /// The line leading from `node` towards the router, if any.
    pub(crate) fn uplink(&self, node: Node) -> Option<Connection> {
        let host = match node {
            Node::Device(id) => self.device(id)?.guest.map(|g| g.host),
            Node::Card(_) => None,
        };
        let (upstream, kind) = if let Some(host) = host {
            (Node::Device(host), ConnectionKind::Known)
        } else if let Some(card) = self.hanging_card(node) {
            let link = self.link(card)?;
            (self.node_of_card(link.upstream)?, link.kind)
        } else {
            // A ghost plugged in nowhere hangs from its own device; a device is a top.
            let Node::Card(card) = node else { return None };
            let owner = self.card_owner(card).filter(|_| self.has_ghost(card))?;
            (Node::Device(owner.id), ConnectionKind::Known)
        };
        Some(Connection {
            upstream,
            downstream: node,
            kind,
        })
    }

    /// The device `id` runs on, for a guest.
    pub(crate) fn host(&self, id: DeviceId) -> Option<DeviceId> {
        Some(self.device(id)?.guest?.host)
    }

    /// The lines of the map's tree. Those through links come first, in the order the links were
    /// found or made, so a new link comes last. Then come those to guests, and then those to
    /// ghosts plugged in nowhere.
    pub(crate) fn connections(&self) -> Vec<Connection> {
        let linked = self.links.iter().filter_map(|l| {
            let node = self.node_of_card(l.card)?;
            (self.hanging_card(node) == Some(l.card))
                .then(|| self.uplink(node))
                .flatten()
        });
        let hosted = self.devices.iter().filter_map(|d| {
            Some(Connection {
                upstream: Node::Device(d.guest?.host),
                downstream: Node::Device(d.id),
                kind: ConnectionKind::Known,
            })
        });
        let nodes = self.nodes().into_iter();
        let loose = nodes.filter_map(|node| match node {
            Node::Card(card) if self.link(card).is_none() => self.uplink(node),
            Node::Card(_) | Node::Device(_) => None,
        });
        linked.chain(hosted).chain(loose).collect()
    }

    /// The nodes hanging straight from `node`, in the order of [`Self::connections`].
    pub(crate) fn behind(&self, node: Node) -> impl Iterator<Item = Node> {
        let lines = self.connections().into_iter();
        lines
            .filter(move |c| c.upstream == node)
            .map(|c| c.downstream)
    }

    /// How many nodes the map can have at most: a chain of uplinks longer than this has a cycle.
    fn node_bound(&self) -> usize {
        self.devices.iter().map(|d| d.cards.len() + 1).sum()
    }

    /// The nodes from `node` up to the top of its tree, `node` first.
    fn ancestry(&self, node: Node) -> impl Iterator<Item = Node> {
        let chain = iter::successors(Some(node), |&at| Some(self.uplink(at)?.upstream));
        chain.take(self.node_bound() + 1)
    }

    /// Whether `node` is `branch` itself or hangs somewhere behind it.
    pub(crate) fn is_within(&self, node: Node, branch: Node) -> bool {
        self.ancestry(node).any(|at| at == branch)
    }

    /// Whether `node` is `device`, one of its ghosts, or hangs behind either. Plugging a card of
    /// `device` into such a node would form a loop.
    pub(crate) fn within_device(&self, node: Node, device: DeviceId) -> bool {
        self.ancestry(node)
            .any(|at| self.node_device(at).is_some_and(|d| d.id == device))
    }

    /// The card that moves when `node` is plugged in elsewhere: for a ghost, that is its card;
    /// for a device, the card it hangs from, or else its first. `None` for a guest, which stays
    /// on its host.
    fn plug_of(&self, node: Node) -> Option<CardId> {
        match node {
            Node::Card(card) => Some(card),
            Node::Device(id) => {
                let device = self.device(id).filter(|d| d.guest.is_none())?;
                let primary = device.primary_card().map(|c| c.id);
                self.tree_card(device).or(primary)
            }
        }
    }

    /// Hangs `node` from `upstream`, as a link the user made from the card it moves (see
    /// [`Self::plug_of`]) into `upstream`'s socket. On `None` it unplugs: a device is left
    /// standing alone, and a ghost goes back under its own device, as it also does when dropped
    /// on that device. The nodes behind it go with it. Refused for a guest, which stays on its
    /// host, and where it would form a loop. A node that stays behind the same upstream keeps
    /// the link it had.
    pub(crate) fn connect(&mut self, node: Node, upstream: Option<Node>) {
        let Some(card) = self.plug_of(node) else {
            return;
        };
        // A ghost dropped on its own device goes back under it, unplugged.
        let own_device = match node {
            Node::Card(_) => self.card_owner(card).map(|d| Node::Device(d.id)),
            Node::Device(_) => None,
        };
        let upstream = upstream.filter(|&up| Some(up) != own_device);
        let unchanged = match upstream {
            Some(up) => self.uplink(node).is_some_and(|l| l.upstream == up),
            None => self.link(card).is_none(),
        };
        if unchanged {
            return;
        }
        let target = match upstream {
            Some(up) => match self.socket(up) {
                Some(socket) => Some(socket),
                None => return,
            },
            None => None,
        };
        self.plug(card, target, false);
    }

    /// Plugs `card` into `target`, or unplugs it on `None`, as a known link. A link `kept` is
    /// also kept when the layout is reset. Refused (false) for a guest's first card, for a
    /// missing card, and where it would form a loop.
    pub(super) fn plug(&mut self, card: CardId, target: Option<CardId>, kept: bool) -> bool {
        let Some(owner) = self.card_owner(card) else {
            return false;
        };
        let guests_first =
            owner.guest.is_some() && owner.primary_card().map(|c| c.id) == Some(card);
        let owner = owner.id;
        let looped = target.is_some_and(|t| {
            self.node_of_card(t)
                .is_none_or(|node| self.within_device(node, owner))
        });
        if guests_first || looped {
            return false;
        }
        let relink = |links: &mut Vec<CardLink>| {
            links.retain(|l| l.card != card);
            if let Some(upstream) = target {
                links.push(CardLink {
                    upstream,
                    card,
                    kind: ConnectionKind::Known,
                });
            }
        };
        relink(&mut self.links);
        if kept {
            relink(&mut self.detected);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;

    const ROUTER: Node = Node::Device(DeviceId(1));
    /// The router's two cards: the home network hangs from the first.
    const ROUTER_LAN: Node = Node::Card(CardId(1));
    const ROUTER_WAN: Node = Node::Card(CardId(101));
    const SWITCH: Node = Node::Device(DeviceId(2));
    const ACCESS_POINT: Node = Node::Device(DeviceId(3));
    const NAS: Node = Node::Device(DeviceId(4));
    const PC: Node = Node::Device(DeviceId(5));
    const LINUX_SERVER: Node = Node::Device(DeviceId(6));
    /// A virtual machine on the Linux Server, with a further card on the lab network.
    const DOCKER_HOST: Node = Node::Device(DeviceId(19));
    const DOCKER_LAB_CARD: Node = Node::Card(CardId(100));
    const LAB_ROUTER: Node = Node::Device(DeviceId(22));

    #[test]
    fn each_card_of_the_router_is_a_ghost_heading_what_is_plugged_into_it() {
        let network = mock_data::network();
        let behind = |node| network.behind(node).collect::<Vec<_>>();
        assert_eq!(behind(ROUTER), [ROUTER_LAN, ROUTER_WAN]);
        assert_eq!(behind(ROUTER_LAN), [NAS, SWITCH, ACCESS_POINT]);
        assert!(behind(ROUTER_WAN).is_empty());
        let nodes = network.nodes();
        assert_eq!(
            nodes.get(..3),
            Some([ROUTER, ROUTER_LAN, ROUTER_WAN].as_slice())
        );
        assert_eq!(
            nodes.iter().filter(|n| matches!(n, Node::Card(_))).count(),
            3,
            "the router's two and the Docker Host's lab card"
        );
    }

    #[test]
    fn a_guest_hangs_from_its_host_and_its_further_card_where_it_is_plugged_in() {
        let network = mock_data::network();
        let uplink = network.uplink(DOCKER_HOST);
        assert_eq!(uplink.map(|c| c.upstream), Some(LINUX_SERVER));
        let lab = network.uplink(DOCKER_LAB_CARD);
        assert_eq!(lab.map(|c| c.upstream), Some(LAB_ROUTER));
        assert!(network.behind(LAB_ROUTER).any(|n| n == DOCKER_LAB_CARD));
        assert!(network.behind(DOCKER_HOST).all(|n| n != DOCKER_LAB_CARD));
    }

    #[test]
    fn a_device_moves_to_another_line_with_its_branch() {
        let mut network = mock_data::network();
        network.connect(PC, Some(ROUTER_LAN));
        let link = network.uplink(PC);
        assert_eq!(
            link.map(|l| (l.upstream, l.kind)),
            Some((ROUTER_LAN, ConnectionKind::Known))
        );
        let to_pc = network
            .connections()
            .into_iter()
            .filter(|c| c.downstream == PC);
        assert_eq!(to_pc.count(), 1);

        network.connect(SWITCH, Some(ACCESS_POINT));
        assert!(network.is_within(PC, ROUTER));
        assert!(!network.is_within(PC, SWITCH), "the PC left the switch");
        assert!(
            network.is_within(LINUX_SERVER, ACCESS_POINT),
            "the switch kept its devices"
        );
        assert!(
            network.is_within(DOCKER_HOST, ACCESS_POINT),
            "and their guests"
        );
    }

    #[test]
    fn a_new_link_comes_last_and_an_unchanged_one_stays_as_found() {
        let mut network = mock_data::network();
        let behind = |n: &Network| n.behind(ROUTER_LAN).collect::<Vec<_>>();
        network.connect(PC, Some(ROUTER_LAN));
        assert_eq!(behind(&network), [NAS, SWITCH, ACCESS_POINT, PC]);

        let phone = Node::Device(DeviceId(10));
        let found = network.uplink(phone);
        network.connect(phone, Some(ACCESS_POINT));
        assert_eq!(network.uplink(phone), found);
        assert_eq!(found.map(|l| l.kind), Some(ConnectionKind::Inferred));
    }

    #[test]
    fn nothing_joins_its_own_branch_or_a_device_with_no_socket() {
        let mut network = mock_data::network();
        let before = network.connections();
        network.connect(SWITCH, Some(PC));
        network.connect(SWITCH, Some(SWITCH));
        network.connect(SWITCH, Some(Node::Device(DeviceId(999))));
        network.connect(PC, Some(ROUTER));
        network.connect(ROUTER_LAN, Some(PC));
        assert_eq!(network.connections(), before);
    }

    #[test]
    fn a_guest_stays_on_its_host() {
        let mut network = mock_data::network();
        let before = network.connections();
        network.connect(DOCKER_HOST, Some(ROUTER_LAN));
        network.connect(DOCKER_HOST, None);
        assert_eq!(network.connections(), before);
    }

    #[test]
    fn a_ghost_unplugged_or_dropped_on_its_own_device_goes_back_under_it() {
        let mut network = mock_data::network();
        network.connect(DOCKER_LAB_CARD, Some(DOCKER_HOST));
        assert_eq!(
            network.uplink(DOCKER_LAB_CARD).map(|c| c.upstream),
            Some(DOCKER_HOST)
        );
        network.connect(DOCKER_LAB_CARD, Some(Node::Device(DeviceId(23))));
        assert_eq!(
            network.uplink(DOCKER_LAB_CARD).map(|c| c.upstream),
            Some(Node::Device(DeviceId(23))),
            "plugged into the Lab Pi"
        );
        network.connect(DOCKER_LAB_CARD, None);
        assert_eq!(
            network.uplink(DOCKER_LAB_CARD).map(|c| c.upstream),
            Some(DOCKER_HOST)
        );
    }

    #[test]
    fn a_device_left_alone_heads_its_own_branch_until_restored() {
        let mut network = mock_data::network();
        let before = network.connections();
        network.connect(SWITCH, None);
        assert!(network.uplink(SWITCH).is_none());
        assert!(network.is_within(PC, SWITCH));
        network.restore_connections();
        assert_eq!(network.connections(), before);
    }

    #[test]
    fn uplinks_lead_to_the_routers() {
        let network = mock_data::network();
        let tops: Vec<_> = network
            .nodes()
            .into_iter()
            .filter(|&n| network.uplink(n).is_none())
            .collect();
        assert_eq!(tops, [ROUTER, LAB_ROUTER]);
    }
}
