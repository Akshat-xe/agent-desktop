use super::*;

fn seed_skeleton_refmap_on_window(window_id: Option<&'static str>) -> RefMap {
    let src = crate::refs::ref_alloc_source::RefAllocSource {
        window_id,
        ..source("TestApp")
    };
    let mut map = RefMap::new();
    let anchor = ref_entry_from_node(&drilled("group", "Sidebar"), &src, None, &[0]);
    let _ = map.allocate(anchor);
    let other = ref_entry_from_node(&drilled("button", "Toolbar"), &src, None, &[1]);
    let _ = map.allocate(other);
    map
}

#[test]
fn test_run_from_ref_resolves_source_window_among_same_process_windows() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_latest(seed_skeleton_refmap_on_window(Some("w-467")));

    let subtree = named("button", "Save");
    let mut adapter = StubAdapter::with_windows(
        subtree,
        vec![window_info("w-467", false), window_info("w-475", false)],
    );
    adapter.expected_app = Some("TestApp".into());

    let result = run_from_ref(&adapter, &drill_opts(), "@e1", Some(&snapshot_id))
        .expect("drill must resolve its own source window, not the process");
    assert_eq!(result.window.id, "w-467");
}

#[test]
fn test_run_from_ref_does_not_persist_refs_when_source_window_is_gone() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_latest(seed_skeleton_refmap_on_window(Some("w-gone")));

    let subtree = named("button", "Save");
    let adapter = StubAdapter::with_windows(subtree, vec![window_info("w-475", true)]);

    let result = run_from_ref(&adapter, &drill_opts(), "@e1", Some(&snapshot_id));
    let err = match result {
        Ok(_) => panic!("drill must fail when its source window is gone"),
        Err(e) => e,
    };
    match err {
        AppError::Adapter(adapter_err) => {
            assert_eq!(adapter_err.code, crate::ErrorCode::WindowNotFound);
        }
        other => panic!("expected Adapter(WindowNotFound), got {other:?}"),
    }
    let on_disk = load_latest();
    assert_eq!(
        on_disk.len(),
        2,
        "seed refs only; a failed drill must not persist refs"
    );
}

#[test]
fn test_run_from_ref_drills_process_owned_ref_on_any_surface() {
    for surface in [
        crate::SnapshotSurface::Window,
        crate::SnapshotSurface::Menubar,
    ] {
        let _guard = HomeGuard::new();
        let src = crate::refs::ref_alloc_source::RefAllocSource {
            window_title: Some(""),
            surface,
            ..source("TestApp")
        };
        let mut anchor = ref_entry_from_node(&named("group", "Dock"), &src, None, &[0]);
        anchor.identity.unique_at_capture = true;
        let mut map = RefMap::new();
        let _ = map.allocate(anchor);
        let snapshot_id = save_latest(map);

        let adapter = StubAdapter::with_windows(named("group", "Dock"), vec![]);
        let result = run_from_ref(&adapter, &drill_opts(), "@e1", Some(&snapshot_id)).unwrap();
        assert!(result.window.is_process_owner(), "{surface:?}");
    }
}
