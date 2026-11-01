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
