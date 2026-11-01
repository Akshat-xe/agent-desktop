use std::sync::atomic::Ordering;

use super::tokenless_capture_support::*;
use crate::ErrorCode;
use crate::refs::refs_test_support::HomeGuard;
use crate::snapshot::snapshot_ref::run_from_ref;

#[test]
fn a_skeleton_anchor_resolves_and_its_drill_resolves() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(anchored_window());

    let captured = snapshot(&adapter);
    assert_eq!(
        adapter.captures(),
        2,
        "the shallow walk decides, the whole window proves"
    );
    let (anchor_ref, anchor) = named(&captured.refmap, "Sidebar");
    assert!(
        anchor.identity.unique_at_capture,
        "an inert anchor is counted by the capture census too"
    );
    assert_eq!(captured.refmap.len(), 1);
    let sidebar = &captured.tree.children[0].children[0].children[0];
    assert_eq!(sidebar.children_count, Some(1), "a cut keeps its count");

    let snapshot_id = save(&captured.refmap);
    let before = adapter.captures();
    let drilled = run_from_ref(&adapter, &drill_opts(), &anchor_ref, Some(&snapshot_id))
        .expect("a tokenless drill reuses the walk its root resolved through");
    assert_eq!(adapter.captures(), before + 1, "one walk, not two");
    assert_eq!(
        adapter.subtrees.load(Ordering::SeqCst),
        0,
        "a tokenless drill never reads the subtree again"
    );

    let (_, button) = named(&drilled.refmap, "Save");
    assert!(
        button.identity.unique_at_capture,
        "the drill counted its refs against the window it will be resolved in"
    );
    assert_eq!(button.identity.role, "button");

    resolve(&adapter, button).expect("the drilled ref resolves");
    let anchored = adapter.anchored();
    assert_eq!(anchored.len(), 1);
    assert_eq!(anchored[0].scope.path.as_slice(), &[0, 0, 0, 0, 0]);
}

#[test]
fn a_twin_below_the_skeleton_depth_makes_the_anchor_not_unique() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(twinned_window());

    let captured = snapshot(&adapter);
    let (anchor_ref, anchor) = named(&captured.refmap, "Sidebar");
    assert!(
        !anchor.identity.unique_at_capture,
        "a twin the shallow walk cannot see still makes the anchor ambiguous"
    );
    let (_, toolbar) = named(&captured.refmap, "Toolbar");
    assert!(
        toolbar.identity.unique_at_capture,
        "the census that refuses the twin still proves its neighbour"
    );

    let refused = refusal(&adapter, anchor);
    assert_eq!(refused.code, ErrorCode::AmbiguousTarget);
    assert_eq!(
        refused.details.expect("details")["kind"],
        "identity_not_unique"
    );
    resolve(&adapter, toolbar).expect("a proven anchor resolves");

    let snapshot_id = save(&captured.refmap);
    let drill = match run_from_ref(&adapter, &drill_opts(), &anchor_ref, Some(&snapshot_id)) {
        Ok(_) => panic!("an ambiguous anchor cannot be drilled either"),
        Err(error) => error,
    };
    match drill {
        crate::AppError::Adapter(error) => {
            assert_eq!(error.code, ErrorCode::AmbiguousTarget);
            assert_eq!(
                error.details.expect("details")["kind"],
                "identity_not_unique"
            );
        }
        other => panic!("expected an adapter refusal, got {other:?}"),
    }
}

#[test]
fn a_drill_under_a_moved_root_records_the_live_path() {
    let _guard = HomeGuard::new();
    let adapter = CaptureAdapter::new(anchored_window());

    let captured = snapshot(&adapter);
    let (anchor_ref, _) = named(&captured.refmap, "Sidebar");
    assert_eq!(
        captured
            .refmap
            .get(&anchor_ref)
            .expect("anchor")
            .scope
            .path
            .as_slice(),
        &[0, 0, 0]
    );
    let snapshot_id = save(&captured.refmap);

    adapter.show(shifted_window());
    let first = run_from_ref(&adapter, &drill_opts(), &anchor_ref, Some(&snapshot_id))
        .expect("the moved anchor resolves");
    let (first_button, button) = named(&first.refmap, "Save");
    assert_eq!(
        button.scope.path.as_slice(),
        &[0, 1, 0, 0, 0],
        "a drilled ref is recorded in the path of the node the drill matched"
    );

    let second = run_from_ref(&adapter, &drill_opts(), &anchor_ref, Some(&snapshot_id))
        .expect("the re-drill replaces one subtree");
    let (second_button, _) = named(&second.refmap, "Save");
    assert!(
        second.refmap.get(&first_button).is_none(),
        "the previous drill's refs are pruned through their root, not by path"
    );
    assert_ne!(first_button, second_button);
    assert!(
        second.refmap.get(&anchor_ref).is_some(),
        "the drill never removes its own root"
    );
}
