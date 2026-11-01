use super::identity_test_support::*;
use super::*;
use crate::ErrorCode;
use crate::refs::refs_test_support::HomeGuard;

#[test]
fn drilled_ref_ignores_an_outside_twin_and_never_chooses_it() {
    let _home = HomeGuard::new();
    let (id, _) = save_map();
    let adapter = DrillAdapter::new();
    adapter
        .tree
        .lock()
        .unwrap()
        .children
        .push(node("button", "Save"));
    click(&adapter, &format!("@{id}:e2")).unwrap();
    assert_eq!(*adapter.actions.lock().unwrap(), vec![vec![0, 0]]);
    adapter.tree.lock().unwrap().children[0]
        .children
        .push(node("button", "Save"));
    let error = click(&adapter, &format!("@{id}:e2")).unwrap_err();
    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
    assert_eq!(adapter.actions.lock().unwrap().len(), 1);
    adapter.tree.lock().unwrap().children[0].children.clear();
    let error = click(&adapter, &format!("@{id}:e2")).unwrap_err();
    assert_eq!(error.code(), "STALE_REF");
    assert_eq!(adapter.actions.lock().unwrap().len(), 1);
}

#[test]
fn incomplete_drill_does_not_replace_saved_refs() {
    let _home = HomeGuard::new();
    let (id, map) = save_map();
    let adapter = DrillAdapter::new();
    adapter.tree.lock().unwrap().children[0].subtree_truncated = true;
    let error = click(&adapter, &format!("@{id}:e2")).unwrap_err();
    assert_eq!(error.code(), "TIMEOUT");
    assert!(adapter.actions.lock().unwrap().is_empty());
    let error = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id))
        .err()
        .unwrap();
    assert_eq!(error.code(), ErrorCode::Timeout.as_str());
    let saved = RefStore::new().unwrap().load_snapshot(&id).unwrap();
    assert_eq!(saved.get("@e2"), map.get("@e2"));
}

#[test]
fn moved_root_cannot_redirect_hydration_to_a_twin_at_the_old_path() {
    let _home = HomeGuard::new();
    let (id, _) = save_map();
    let adapter = DrillAdapter::new();
    let mut outside = node("group", "Other");
    outside.children.push(node("button", "Save"));
    adapter.tree.lock().unwrap().children.insert(0, outside);
    let error = click(&adapter, &format!("@{id}:e2")).unwrap_err();
    assert_eq!(error.code(), "STALE_REF");
    assert!(adapter.actions.lock().unwrap().is_empty());
    assert!(error.suggestion().unwrap().contains("Drill the root"));

    let adapter = DrillAdapter::new();
    adapter.tree.lock().unwrap().children = vec![node("button", "Save")];
    let error = click(&adapter, &format!("@{id}:e2")).unwrap_err();
    assert_eq!(error.code(), "STALE_REF");
    assert!(adapter.actions.lock().unwrap().is_empty());
    assert!(error.suggestion().unwrap().contains("Drill the root"));
}

#[test]
fn legacy_skeleton_anchor_cannot_be_drilled() {
    let _home = HomeGuard::new();
    let (_, map) = save_map();
    let adapter = DrillAdapter::new();
    let mut legacy = map.get("@e1").unwrap().clone();
    legacy.identity.basis = None;
    let mut map = crate::refs::RefMap::new();
    map.allocate(legacy);
    let id = RefStore::new().unwrap().save_new_snapshot(&map).unwrap();
    let error = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id))
        .err()
        .unwrap();
    assert_eq!(error.code(), "STALE_REF");
    assert!(adapter.actions.lock().unwrap().is_empty());
}

#[test]
fn drilled_ref_poll_and_dispatch_use_the_original_snapshot() {
    let _home = HomeGuard::new();
    let (id, map) = save_map();
    RefStore::new()
        .unwrap()
        .save_new_snapshot(&crate::refs::RefMap::new())
        .unwrap();
    let adapter = DrillAdapter::new();
    adapter
        .tree
        .lock()
        .unwrap()
        .children
        .push(node("button", "Save"));
    crate::ref_action::ref_action_wait::execute_with_auto_wait(
        crate::ref_action::ref_action_wait_context::RefActionWaitContext {
            adapter: &adapter,
            entry: map.get("@e2").unwrap(),
            ref_id: &format!("@{id}:e2"),
            context: &crate::CommandContext::default(),
        },
        crate::ActionRequest::headless(crate::Action::Click).with_timeout_ms(Some(500)),
        crate::ref_action::dispatch_resolved,
    )
    .unwrap();
    assert_eq!(*adapter.actions.lock().unwrap(), vec![vec![0, 0]]);
}

#[test]
fn nested_drill_resolves_each_root_inside_its_parent() {
    let _home = HomeGuard::new();
    let (_, saved) = save_map();
    let mut map = crate::refs::RefMap::new();
    map.allocate(saved.get("@e1").unwrap().clone());
    let mut inner = saved.get("@e1").unwrap().clone();
    inner.identity.name = Some("Inner".into());
    inner.scope.root_ref = Some("@e1".into());
    inner.scope.path.push(0);
    inner.scope.path_is_absolute = true;
    map.allocate(inner);
    let mut target = saved.get("@e2").unwrap().clone();
    target.scope.root_ref = Some("@e2".into());
    target.scope.path.push(0);
    map.allocate(target);
    let id = RefStore::new().unwrap().save_new_snapshot(&map).unwrap();
    let adapter = DrillAdapter::new();
    let mut inner = node("group", "Inner");
    inner.children.push(node("button", "Save"));
    adapter.tree.lock().unwrap().children[0].children = vec![inner.clone()];
    adapter.tree.lock().unwrap().children.push(inner);
    click(&adapter, &format!("@{id}:e3")).unwrap();
    assert_eq!(*adapter.actions.lock().unwrap(), vec![vec![0, 0, 0]]);
    adapter
        .replace_outer_on_target
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(
        click(&adapter, &format!("@{id}:e3")).unwrap_err().code(),
        "STALE_REF"
    );
    assert_eq!(adapter.actions.lock().unwrap().len(), 1);
}

#[test]
fn cyclic_or_missing_roots_are_stale() {
    let _home = HomeGuard::new();
    for root_ref in ["@e1", "@e99"] {
        let (_, saved) = save_map();
        let mut map = crate::refs::RefMap::new();
        let mut root = saved.get("@e1").unwrap().clone();
        root.scope.root_ref = Some(root_ref.into());
        map.allocate(root);
        map.allocate(saved.get("@e2").unwrap().clone());
        let id = RefStore::new().unwrap().save_new_snapshot(&map).unwrap();
        let adapter = DrillAdapter::new();
        assert_eq!(
            click(&adapter, &format!("@{id}:e2")).unwrap_err().code(),
            "STALE_REF"
        );
        assert!(adapter.actions.lock().unwrap().is_empty());
    }
}

#[test]
fn drilling_a_root_that_moved_is_stale_and_allocates_no_refs() {
    let _home = HomeGuard::new();
    let (id, map) = save_map();
    let adapter = DrillAdapter::new();
    adapter
        .tree
        .lock()
        .unwrap()
        .children
        .insert(0, node("group", "Other"));
    let error = run_from_ref(&adapter, &TreeOptions::default(), "@e1", Some(&id))
        .err()
        .expect("a moved root must not be drilled");
    assert_eq!(error.code(), "STALE_REF");
    assert_eq!(
        error.suggestion(),
        Some("Take a new snapshot, then drill again.")
    );
    let stored = crate::refs::refs_store::RefStore::new()
        .unwrap()
        .load_snapshot(&id)
        .unwrap();
    assert_eq!(stored.len(), map.len());
}
