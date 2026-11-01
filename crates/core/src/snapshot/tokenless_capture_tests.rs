use std::sync::atomic::Ordering;

use super::tokenless_capture_support::*;
use crate::locator::live_locator::ObservationRequest;
use crate::refs::refs_test_support::HomeGuard;
use crate::{AccessibilityNode, AdapterError, Deadline, ErrorCode, RefMap};

fn entries(refmap: &RefMap) -> Vec<&crate::RefEntry> {
    (1..=64)
        .filter_map(|index| refmap.get(&format!("@e{index}")))
        .collect()
}

fn cut_short(
    _request: &ObservationRequest,
    mut tree: AccessibilityNode,
) -> Result<AccessibilityNode, AdapterError> {
    let mut unread = node("scrollarea", "Unread");
    unread.children_count = Some(3);
    unread.subtree_truncated = true;
    tree.children.push(unread);
    Ok(tree)
}

fn faulted(
    _request: &ObservationRequest,
    _tree: AccessibilityNode,
) -> Result<AccessibilityNode, AdapterError> {
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "a node below the shallow depth faulted",
    ))
}

fn needs_forced_renderer(
    request: &ObservationRequest,
    tree: AccessibilityNode,
) -> Result<AccessibilityNode, AdapterError> {
    if request.observation_mode.force_renderer_accessibility {
        return Ok(tree);
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "the renderer tree is thin until accessibility is forced",
    ))
}

#[test]
fn a_whole_window_read_cut_short_keeps_the_complete_shallow_tree() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(anchored_window());
    adapter.fault_whole_read(cut_short);

    let captured = snapshot(&adapter);
    assert_eq!(adapter.captures(), 2);
    assert!(captured.complete, "the shallow walk finished");
    let tree = serde_json::to_string(&captured.tree).expect("the tree serializes");
    assert!(
        !tree.contains("Unread"),
        "the cut-short read is not presented"
    );
    assert!(
        entries(&captured.refmap).is_empty(),
        "a read that was cut short proves nothing, so no anchor is handed out"
    );
}

#[test]
fn a_failed_whole_window_read_keeps_the_shallow_tree() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(anchored_window());
    adapter.fault_whole_read(faulted);

    let captured = snapshot(&adapter);
    assert!(captured.complete, "the shallow walk finished");
    assert!(entries(&captured.refmap).is_empty());
}

#[test]
fn the_whole_window_read_carries_the_forced_renderer_hint() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(anchored_window());
    adapter.fault_whole_read(needs_forced_renderer);
    let forced = crate::adapter::TreeOptions {
        force_renderer_accessibility: true,
        ..skeleton_opts()
    };

    let captured = crate::snapshot::build(
        &adapter,
        &forced,
        Some("Fixture"),
        None,
        Deadline::standard().expect("a standard deadline"),
    )
    .expect("the window snapshots");
    let (_, anchor) = named(&captured.refmap, "Sidebar");
    assert!(
        anchor.identity.unique_at_capture,
        "the forced read is adopted and proves the anchor"
    );
}

#[test]
fn an_incomplete_window_offers_no_drill_anchor() {
    let _guard = HomeGuard::new();
    let mut hidden = node("scrollarea", "Feed");
    hidden.children_count = Some(3);
    hidden.subtree_truncated = true;
    let mut sidebar = node("group", "Sidebar");
    sidebar.children = vec![group(vec![node("button", "Save")])];
    let adapter = CaptureAdapter::new(window(vec![group(vec![group(vec![sidebar, hidden])])]));

    let captured = snapshot(&adapter);
    assert!(!captured.complete, "the window was not read whole");
    assert_eq!(
        captured.refmap.len(),
        0,
        "an unread region proves nothing, so no anchor is handed out"
    );
}

#[test]
fn a_shallow_capture_of_tokened_refs_takes_one_walk() {
    let _guard = HomeGuard::new();
    let mut run = node("button", "Run");
    run.identity.retained_object = Some(TOKEN.into());
    let mut deep = node("group", "");
    deep.identity.retained_object = Some("axtok1:deep".into());
    deep.children = vec![node("button", "Deep")];
    let adapter = CaptureAdapter::new(window(vec![run, group(vec![group(vec![deep])])]));

    let captured = snapshot(&adapter);
    assert_eq!(
        adapter.captures(),
        1,
        "a ref that re-identifies itself by token needs no second walk"
    );
    let (_, ref_entry) = named(&captured.refmap, "Run");
    assert_eq!(ref_entry.identity.retained_object.as_deref(), Some(TOKEN));

    resolve(&adapter, ref_entry).expect("a token resolves through the adapter");
    assert_eq!(adapter.strict.load(Ordering::SeqCst), 1);
    assert!(adapter.anchored().is_empty());
}
