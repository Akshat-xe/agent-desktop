use super::identity_test_support::*;
use super::*;
use crate::refs::refs_test_support::HomeGuard;
use crate::{IdentityBasis, ProcessId};

struct UngatedAdapter;

impl crate::ObservationOps for UngatedAdapter {
    fn resolve_element_strict(
        &self,
        _: &crate::RefEntry,
        _: crate::Deadline,
    ) -> Result<crate::NativeHandle, crate::AdapterError> {
        Ok(crate::NativeHandle::null())
    }
}
impl crate::ActionOps for UngatedAdapter {}
impl crate::InputOps for UngatedAdapter {}
impl crate::SystemOps for UngatedAdapter {}

fn capture_skeleton(names: &[&str]) -> (String, crate::refs::RefMap) {
    capture_items("group", names)
}

fn capture_items(role: &str, names: &[&str]) -> (String, crate::refs::RefMap) {
    let mut window = node("window", "Test");
    window.children = names
        .iter()
        .map(|name| {
            let mut anchor = node(role, name);
            anchor.children_count = Some(3);
            if role == "treeitem" {
                anchor.presentation.available_actions = vec!["Click".into()];
            }
            anchor
        })
        .collect();
    let config = RefAllocConfig {
        options: RefAllocOptions {
            include_bounds: false,
            interactive_only: false,
            compact: false,
            skeleton_depth: Some(3),
        },
        source: RefAllocSource {
            pid: ProcessId::new(42),
            app: Some("TestApp"),
            window_id: Some("w-42"),
            window_title: Some("Test"),
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
        complete: true,
    };
    let mut map = crate::refs::RefMap::new();
    ref_alloc::allocate_refs(window, &mut map, &config).unwrap();
    let id = RefStore::new().unwrap().save_new_snapshot(&map).unwrap();
    (id, map)
}

fn adapter_with(names: &[&str]) -> DrillAdapter {
    let adapter = DrillAdapter::new();
    let mut window = node("window", "Test");
    window.children = names
        .iter()
        .map(|name| {
            let mut anchor = node("group", name);
            anchor.children.push(node("button", "Save"));
            anchor
        })
        .collect();
    *adapter.tree.lock().unwrap() = window;
    adapter
}

#[test]
fn removed_twin_never_lets_a_drill_select_the_survivor() {
    let _home = HomeGuard::new();
    let (id, map) = capture_skeleton(&["Panel", "Panel"]);
    assert_eq!(
        map.get("@e1").unwrap().identity.basis,
        Some(IdentityBasis::Ambiguous)
    );
    let adapter = adapter_with(&["Panel"]);
    let error = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id))
        .err()
        .unwrap();
    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
}

#[test]
fn skeleton_twin_refuses_the_drill_while_both_exist() {
    let _home = HomeGuard::new();
    let (id, _) = capture_skeleton(&["Panel", "Panel"]);
    let adapter = adapter_with(&["Panel", "Panel"]);
    let error = run_from_ref(&adapter, &TreeOptions::default(), "@e2", Some(&id))
        .err()
        .unwrap();
    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
}

#[test]
fn unique_skeleton_anchor_drills_and_its_child_acts_on_the_right_element() {
    let _home = HomeGuard::new();
    let (id, map) = capture_skeleton(&["Panel", "Other"]);
    assert_eq!(
        map.get("@e1").unwrap().identity.basis,
        Some(IdentityBasis::Name)
    );
    let adapter = adapter_with(&["Panel", "Other"]);
    let result = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id)).unwrap();
    let child = result.tree.children[0].ref_id.clone().unwrap();
    click(&adapter, &child).unwrap();
    assert_eq!(*adapter.actions.lock().unwrap(), vec![vec![0, 0]]);
}

fn with_deep_twin_at(adapter: &DrillAdapter, host: usize) {
    let mut deep = node("group", "Panel");
    for _ in 0..3 {
        let mut parent = node("group", "Wrap");
        parent.children.push(deep);
        deep = parent;
    }
    adapter.tree.lock().unwrap().children[host]
        .children
        .push(deep);
}

#[test]
fn a_twin_below_the_skeleton_depth_cannot_stand_in_for_a_removed_anchor() {
    let _home = HomeGuard::new();
    let (id, map) = capture_skeleton(&["Panel", "Other"]);
    assert_eq!(map.get("@e1").unwrap().scope.census_depth, Some(3));
    let adapter = adapter_with(&["Other"]);
    adapter.tree.lock().unwrap().children[0].children.clear();
    with_deep_twin_at(&adapter, 0);
    let drill = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id));
    assert_eq!(drill.err().unwrap().code(), "STALE_REF");
    let error = click(&adapter, &format!("@{id}:e1")).unwrap_err();
    assert_eq!(error.code(), "STALE_REF");
    assert!(adapter.actions.lock().unwrap().is_empty());
}

#[test]
fn a_twin_below_the_skeleton_depth_does_not_stop_the_present_anchor() {
    let _home = HomeGuard::new();
    let (id, _) = capture_skeleton(&["Panel", "Other"]);
    let adapter = adapter_with(&["Panel", "Other"]);
    with_deep_twin_at(&adapter, 1);
    click(&adapter, &format!("@{id}:e1")).unwrap();
    assert_eq!(*adapter.actions.lock().unwrap(), vec![vec![0]]);
    let result = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id)).unwrap();
    assert!(result.tree.children[0].ref_id.is_some());
}

#[test]
fn legacy_basisless_entry_is_stale_through_the_shared_resolver() {
    let _home = HomeGuard::new();
    let (_, map) = save_map();
    let mut legacy = map.get("@e1").unwrap().clone();
    legacy.identity.basis = None;
    let error = crate::refs::ref_resolve::resolve(
        &UngatedAdapter,
        &legacy,
        "@s1:e1",
        &crate::CommandContext::default(),
        crate::Deadline::standard().unwrap(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, crate::ErrorCode::StaleRef);
}

#[test]
fn actionable_skeleton_item_drills_like_an_anchor_and_refuses_a_deeper_twin() {
    let _home = HomeGuard::new();
    let (id, map) = capture_items("treeitem", &["Recents", "Shared"]);
    let entry = map.get("@e1").unwrap();
    assert_eq!(entry.identity.basis, Some(IdentityBasis::Name));
    assert_eq!(entry.scope.census_depth, Some(3));
    let adapter = DrillAdapter::new();
    adapter
        .derived_names
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let mut window = node("window", "Test");
    window.children = ["Recents", "Shared"]
        .iter()
        .map(|name| {
            let mut item = node("treeitem", name);
            item.children.push(node("button", "Save"));
            item
        })
        .collect();
    *adapter.tree.lock().unwrap() = window;
    run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id)).unwrap();
    adapter.tree.lock().unwrap().children.remove(0);
    let mut deep = node("treeitem", "Recents");
    for _ in 0..3 {
        let mut parent = node("group", "Wrap");
        parent.children.push(deep);
        deep = parent;
    }
    adapter.tree.lock().unwrap().children[0].children.push(deep);
    let error = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id))
        .err()
        .unwrap();
    assert_eq!(error.code(), "STALE_REF");
}
