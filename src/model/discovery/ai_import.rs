//! Import with AI as the run sees it: the new hosts waiting for a decision when it began,
//! identified one at a time and each added to the map once its answer is in. A new run forgets
//! it. Loading the model and asking it are elsewhere (`crate::ai`).

use super::Discovery;
use crate::model::device::{Device, DeviceId};
use crate::model::network::Network;

/// How far Import with AI has got, or how it ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AiImportPhase {
    Loading,
    /// A host is with the model.
    Identifying,
    Finished,
    Stopped,
    /// The model it uses is not installed.
    NoModel,
    /// The model could not load or answer.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AiImport {
    pub(crate) phase: AiImportPhase,
    /// The hosts still to identify, by their place in [`Discovery::found`], next last.
    waiting: Vec<usize>,
    /// The host with the model.
    current: Option<usize>,
    /// Hosts the model has answered for, of all there were.
    pub(crate) done: usize,
    pub(crate) total: usize,
}

impl AiImport {
    pub(crate) fn busy(&self) -> bool {
        matches!(
            self.phase,
            AiImportPhase::Loading | AiImportPhase::Identifying
        )
    }

    /// Keeps the index queue pointing at the same hosts after one result is removed. If that
    /// host was currently with the model, its eventual answer is discarded instead of being
    /// applied to the host that shifted into its place.
    pub(super) fn host_removed(&mut self, removed: usize) {
        self.waiting.retain(|&index| index != removed);
        for index in &mut self.waiting {
            if *index > removed {
                *index -= 1;
            }
        }
        self.current = match self.current {
            Some(index) if index == removed => None,
            Some(index) if index > removed => Some(index - 1),
            current => current,
        };
    }
}

impl Discovery {
    pub(crate) fn ai_import(&self) -> Option<&AiImport> {
        self.ai_import.as_ref()
    }

    /// Import with AI is loading the model or identifying hosts.
    pub(crate) fn ai_import_busy(&self) -> bool {
        self.ai_import.as_ref().is_some_and(AiImport::busy)
    }

    /// Begins Import with AI over the new hosts waiting for a decision, loading the model
    /// first. Refused (false) while a scan or another import is busy, or with nothing to import.
    pub(crate) fn begin_ai_import(&mut self) -> bool {
        if self.busy() || self.ai_import_busy() {
            return false;
        }
        let hosts = self.hosts.iter().enumerate();
        let mut waiting: Vec<usize> = hosts
            .filter(|(_, found)| found.importable())
            .map(|(index, _)| index)
            .collect();
        if waiting.is_empty() {
            return false;
        }
        waiting.reverse();
        self.ai_import = Some(AiImport {
            phase: AiImportPhase::Loading,
            total: waiting.len(),
            waiting,
            current: None,
            done: 0,
        });
        true
    }

    /// The next host for the model, skipping any the user added or ignored meanwhile; with
    /// none left, the import has finished.
    pub(crate) fn next_for_ai(&mut self) -> Option<usize> {
        let hosts = &self.hosts;
        let import = self.ai_import.as_mut().filter(|i| i.busy())?;
        let next = std::iter::from_fn(|| import.waiting.pop())
            .find(|&index| hosts.get(index).is_some_and(super::Found::importable));
        import.current = next;
        import.phase = if next.is_some() {
            AiImportPhase::Identifying
        } else {
            AiImportPhase::Finished
        };
        next
    }

    /// Takes the model's answer for the host it has: `fill` puts it in that host's fields, then
    /// it is added to the map as [`Discovery::add`] does. Returns the device added, if any.
    pub(crate) fn identified(
        &mut self,
        fill: impl FnOnce(&mut Device),
        network: &mut Network,
    ) -> Option<DeviceId> {
        let import = self.ai_import.as_mut().filter(|i| i.busy())?;
        let index = import.current.take()?;
        import.done += 1;
        self.edit(index, fill);
        self.add(index, network)
    }

    /// Ends a busy import as `phase` says, leaving the hosts not yet added for the user.
    pub(crate) fn end_ai_import(&mut self, phase: AiImportPhase) {
        if let Some(import) = self.ai_import.as_mut().filter(|i| i.busy()) {
            import.phase = phase;
            import.current = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock_data;
    use crate::model::{DeviceKind, Found};

    /// A finished run of the sample home network, and the network it ran against.
    fn finished() -> (Discovery, Network) {
        let network = mock_data::network();
        let mut discovery = Discovery::default();
        assert!(discovery.start("192.168.1.0/24".parse().unwrap()));
        for answer in mock_data::answers(&network) {
            discovery.answer(answer, &network);
        }
        assert!(discovery.finish());
        (discovery, network)
    }

    #[test]
    fn each_new_host_is_filled_then_added_in_turn_until_none_are_left() {
        let (mut discovery, mut network) = finished();
        let importable = discovery.importable();
        assert!(importable > 1);
        assert!(discovery.begin_ai_import());
        assert!(!discovery.begin_ai_import(), "one at a time");
        let phase = |d: &Discovery| d.ai_import().map(|i| i.phase);
        assert_eq!(phase(&discovery), Some(AiImportPhase::Loading));

        let first = discovery.next_for_ai().unwrap();
        assert_eq!(phase(&discovery), Some(AiImportPhase::Identifying));
        let fill = |d: &mut Device| d.notes = "From the AI".into();
        let id = discovery.identified(fill, &mut network).unwrap();
        assert_eq!(network.device(id).unwrap().notes, "From the AI");
        assert_eq!(discovery.found()[first].on_map(), Some(id));
        assert_eq!(discovery.identified(fill, &mut network), None, "answered");

        // A host the user ignores meanwhile is skipped.
        let ignored = discovery
            .found()
            .iter()
            .rposition(Found::importable)
            .unwrap();
        discovery.ignore(ignored);
        while discovery.next_for_ai().is_some() {
            discovery.identified(|d| d.kind = DeviceKind::Smart, &mut network);
        }
        let import = discovery.ai_import().unwrap();
        assert_eq!(
            (import.phase, import.done, import.total),
            (AiImportPhase::Finished, importable - 1, importable)
        );
        assert_eq!(discovery.importable(), 0);
    }

    #[test]
    fn a_stopped_import_leaves_the_rest_and_a_new_run_forgets_it() {
        let (mut discovery, mut network) = finished();
        assert!(discovery.begin_ai_import());
        discovery.next_for_ai();
        discovery.end_ai_import(AiImportPhase::Stopped);
        assert_eq!(discovery.identified(|_| {}, &mut network), None);
        assert_eq!(discovery.next_for_ai(), None);
        assert!(discovery.importable() > 0);
        assert!(discovery.begin_ai_import(), "again, once ended");
        assert!(discovery.start("192.168.1.0/24".parse().unwrap()));
        assert_eq!(discovery.ai_import(), None);
        assert!(!discovery.begin_ai_import(), "not while scanning");
    }

    #[test]
    fn removing_a_result_does_not_retarget_queued_or_in_flight_answers() {
        let mut import = AiImport {
            phase: AiImportPhase::Identifying,
            waiting: vec![0, 1, 3, 4],
            current: Some(2),
            done: 0,
            total: 5,
        };
        import.host_removed(1);
        assert_eq!(import.waiting, [0, 2, 3]);
        assert_eq!(import.current, Some(1));
        import.host_removed(1);
        assert_eq!(
            import.current, None,
            "answer for the removed host is discarded"
        );
    }
}
