use std::sync::atomic::Ordering;

use super::tokenless_capture_support::*;
use crate::refs::refs_test_support::HomeGuard;

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
