use super::*;
use crate::snapshot::snapshot_ref::identity_test_support::{DrillAdapter, node, save_map};

#[test]
fn drilled_hover_confirms_the_scoped_receiver_despite_an_outside_twin() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let (id, map) = save_map();
    let adapter = DrillAdapter::new();
    adapter
        .tree
        .lock()
        .unwrap()
        .children
        .push(node("button", "Save"));
    let point = ResolvedPoint {
        point: crate::Point { x: 10.0, y: 10.0 },
        focused: false,
        source_entry: Some(map.get("@e2").unwrap().clone()),
        bounds_hash: None,
    };
    let token = format!("@{id}:e2");
    let context = CommandContext::default();
    confirm_receiver(
        &point,
        crate::Deadline::standard().unwrap(),
        &adapter,
        Some(&token),
        &context,
    )
    .unwrap();
    adapter
        .hit_unknown
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let error = confirm_receiver(
        &point,
        crate::Deadline::standard().unwrap(),
        &adapter,
        Some(&token),
        &context,
    )
    .unwrap_err();
    assert_eq!(error.code(), "ACTION_FAILED");
}

#[test]
fn census_scoped_hover_confirms_the_receiver_despite_a_deeper_twin() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let (_, map) = save_map();
    let adapter = DrillAdapter::new();
    let mut twin_parent = node("group", "Inner");
    twin_parent.children.push(node("button", "Save"));
    adapter.tree.lock().unwrap().children[0]
        .children
        .push(twin_parent);
    let mut entry = map.get("@e2").unwrap().clone();
    entry.scope.root_ref = None;
    entry.scope.census_depth = Some(crate::refs::ref_census::Census {
        depth: 2,
        skeleton: true,
    });
    let point = ResolvedPoint {
        point: crate::Point { x: 10.0, y: 10.0 },
        focused: false,
        source_entry: Some(entry),
        bounds_hash: None,
    };
    confirm_receiver(
        &point,
        crate::Deadline::standard().unwrap(),
        &adapter,
        Some("@s1:e2"),
        &CommandContext::default(),
    )
    .unwrap();
}
