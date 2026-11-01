use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

struct CensusAdapter {
    window_instance: &'static str,
    other_window_focused: bool,
}

impl ObservationOps for CensusAdapter {
    fn list_windows(
        &self,
        _filter: &crate::WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        let mut window = super::super::test_support::window();
        window.pid = crate::ProcessId::new(1);
        window.process_instance = Some(self.window_instance.into());
        Ok(vec![window])
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new(entry.scope.path.to_vec()))
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let raw = match root {
            ObservationRoot::Window(_) => {
                if self.other_window_focused && request.surface == crate::SnapshotSurface::Focused {
                    return Err(AdapterError::new(
                        crate::ErrorCode::WindowNotFound,
                        "focus moved to another window",
                    ));
                }
                let mut raw = crate::adapter::minimal_accessibility_node("window", "Fixture");
                raw.children
                    .push(crate::adapter::minimal_accessibility_node("button", "Save"));
                raw
            }
            ObservationRoot::Element { .. } => {
                crate::adapter::minimal_accessibility_node("button", "Save")
            }
        };
        crate::adapter::observed_tree(&root, raw)
    }
}

impl ActionOps for CensusAdapter {}
impl InputOps for CensusAdapter {}
impl SystemOps for CensusAdapter {}

fn census_entry(surface: crate::SnapshotSurface) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Save"));
    entry.identity.basis = Some(IdentityBasis::Name);
    entry.scope.path = smallvec::smallvec![0];
    entry.scope.census_depth = Some(2);
    entry.source.source_window_id = Some("w-1".into());
    entry.source.source_surface = surface;
    entry
}

fn resolve(adapter: &CensusAdapter, entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    resolve_by_census(adapter, entry, 2, Deadline::standard().unwrap())
}

#[test]
fn a_reused_pid_and_window_id_in_a_new_process_never_resolves() {
    let entry = census_entry(crate::SnapshotSurface::Window);
    let live = CensusAdapter {
        window_instance: "test-instance",
        other_window_focused: false,
    };
    assert!(resolve(&live, &entry).is_ok());
    let replaced = CensusAdapter {
        window_instance: "replacement-instance",
        other_window_focused: false,
    };
    assert_eq!(
        resolve(&replaced, &entry).err().unwrap().code,
        crate::ErrorCode::StaleRef
    );
}

#[test]
fn a_focused_census_ref_replays_over_its_window_after_focus_moves() {
    let entry = census_entry(crate::SnapshotSurface::Focused);
    let adapter = CensusAdapter {
        window_instance: "test-instance",
        other_window_focused: true,
    };
    assert!(resolve(&adapter, &entry).is_ok());
}
