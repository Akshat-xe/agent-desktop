use agent_desktop_core::{
    IdentityBasis, RefCapabilities, RefEntry, RefEntryIdentity, RefGeometry, RefProcess, RefScope,
    RefSource, SnapshotSurface,
};

pub(super) fn core_entry(basis: Option<IdentityBasis>) -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: agent_desktop_core::ProcessId::new(42),
            process_instance: Some("42:100".into()),
        },
        identity: RefEntryIdentity {
            basis,
            retained_object: None,
            role: "button".into(),
            name: Some("Save".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: false,
            census_depth: None,
            path: Default::default(),
        },
    }
}
