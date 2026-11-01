use super::provisional_geometry_candidate;
use agent_desktop_core::{
    ProcessId, RefCapabilities, RefEntry, RefEntryIdentity, RefGeometry, RefProcess, RefScope,
    RefSource, SnapshotSurface,
};

fn bounds_only_entry(retained_object: Option<&str>) -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: ProcessId::new(7),
            process_instance: Some("test-instance".into()),
        },
        identity: RefEntryIdentity {
            retained_object: retained_object.map(String::from),
            role: "cell".into(),
            name: None,
            value: None,
            description: None,
            native_id: None,
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: Some(42),
        },
        capabilities: RefCapabilities {
            states: vec![],
            available_actions: vec![],
        },
        source: RefSource {
            source_app: None,
            source_window_id: Some("w-10".into()),
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::smallvec![0, 1],
        },
    }
}

#[test]
fn bounds_stand_in_for_an_identityless_ref_but_never_replace_a_token() {
    assert!(provisional_geometry_candidate(&bounds_only_entry(None)));
    assert!(!provisional_geometry_candidate(&bounds_only_entry(Some(
        "axtok1:0000000000000000"
    ))));
}
