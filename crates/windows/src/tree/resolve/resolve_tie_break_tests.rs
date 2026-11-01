use crate::tree::resolve::*;
use agent_desktop_core::{LocatorEvidence, LocatorField, refs::RefPath};

use crate::tree::resolve::pair_window::{
    DuplicatePairWindow, NAME_MARKER, PairGeometry, TITLE_MARKER, automation_id, collect_marked,
};
use crate::tree::resolve::resolve_search::{
    NodeAdmission, SEARCH_DESCENT, admit_node, enumerate_children, geometry_contradicts,
};

/// The bounds tie-break returns the stored candidate, not the first one collected, and an identity
/// ref never reaches it: its stored rectangle cannot choose between two live matches.
#[test]
fn the_bounds_tie_break_selects_the_stored_candidate_but_an_identity_ref_stays_ambiguous() {
    crate::tree::fixture::ensure_test_apartment();
    let hosted =
        DuplicatePairWindow::create(PairGeometry::Separated).expect("a duplicate window hosts");
    let deadline = crate::tree::walk::walker_fake::deadline();
    let root = crate::tree::automation::root_from_hwnd(hosted.handle, deadline)
        .expect("the hosted window resolves");
    let source = UiaTreeSource::for_root(&root).expect("a tree source");
    let prepared = source.prepare_root(&root).expect("a prepared root");
    let budget = WalkBudget::new(10, deadline);

    let mut duplicates = Vec::new();
    collect_marked(&source, &prepared, &budget, &mut duplicates);
    assert_eq!(
        duplicates.len(),
        2,
        "the host must present exactly two marked controls"
    );
    let (head, target) = (&duplicates[0], &duplicates[1]);
    let role = head.role.known().cloned().expect("a read role");
    let name = head.name.known().cloned().expect("a read name");
    let native_id = automation_id(head);
    assert_eq!(target.role.known(), Some(&role), "the roles are equal");
    assert_eq!(target.name.known(), Some(&name), "the names are equal");
    assert_eq!(
        automation_id(target),
        native_id,
        "the identifiers are equal, so no identity tier separates the pair"
    );

    let head_hash = crate::tree::resolve::resolve_match::bounds_hash_of(head)
        .expect("the head candidate has a positive-area bounds hash");
    let target_hash = crate::tree::resolve::resolve_match::bounds_hash_of(target)
        .expect("the stored candidate has a positive-area bounds hash");
    assert_ne!(
        head_hash, target_hash,
        "the two controls must occupy distinct rectangles, or the tie-break has nothing to pick by"
    );
    let rect = *target
        .ref_evidence
        .bounds
        .known()
        .expect("a read bounds rectangle");

    let pid = agent_desktop_core::ProcessId::from(std::process::id());
    let mut entry = crate::tree::walk::walker_fake::ref_entry(&role);
    entry.process = agent_desktop_core::RefProcess {
        pid,
        process_instance: Some(
            crate::system::process_identity::token_for_pid(pid)
                .expect("the token read answers")
                .expect("a live process has a token"),
        ),
    };
    entry.identity.name = Some(name);
    entry.identity.native_id = native_id;
    entry.identity.basis = Some(agent_desktop_core::IdentityBasis::NativeId);
    entry.geometry = agent_desktop_core::RefGeometry {
        bounds_space: None,
        bounds: Some(rect),
        bounds_hash: Some(target_hash),
    };
    entry.source.source_window_id = Some(format!("w-{}", hosted.handle));
    entry.source.source_window_title = Some(TITLE_MARKER.into());

    for evidence in &duplicates {
        assert_eq!(
            crate::tree::resolve::resolve_match::candidate_outcome(&entry, evidence),
            CandidateOutcome::Matched,
            "both live controls satisfy every identity tier"
        );
    }
    assert!(
        matches!(
            classify_search(&[Some(head_hash), Some(target_hash)], false, &entry),
            SearchVerdict::Resolved(1)
        ),
        "the stored hash selects the second candidate, so index zero is the wrong answer"
    );

    let error = resolve_element_strict(&entry, deadline)
        .err()
        .expect("two live elements answer to the stored identity, so neither one is the answer");

    assert_eq!(
        error.code,
        ErrorCode::AmbiguousTarget,
        "the stored rectangle must never pick between two equally matching elements: {error:?}"
    );
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("candidate_count"))
            .and_then(serde_json::Value::as_u64),
        Some(2),
        "the count covers every live match, not just the one the hash happens to name"
    );
}

/// Collects the marked controls' evidence together with the resolver's own
/// DFS path to each - the fixture root down to a marked node, one child
/// index per hop - so a test can build a stored ref whose `scope.path` names
/// a specific duplicate rather than only checking what the broad search
/// collects. Mirrors `collect_marked`'s own enumeration
/// (`enumerate_children`), so a path recorded here is the exact path the
/// fast path itself would walk.
fn catalogue_marked(
    source: &UiaTreeSource,
    element: &UIAElement,
    depth: u8,
    budget: &WalkBudget,
    prefix: &mut Vec<usize>,
    out: &mut Vec<(RefPath, LocatorEvidence)>,
) {
    if depth >= 8 {
        return;
    }
    let (_, evidence, _) = source.evidence(element);
    if evidence
        .name
        .known()
        .is_some_and(|name| name == NAME_MARKER)
    {
        let mut path = RefPath::default();
        path.extend_from_slice(prefix);
        out.push((path, evidence));
    }
    let mut ignored = false;
    let Ok(children) = enumerate_children(source, element, budget, &mut ignored, &SEARCH_DESCENT)
    else {
        return;
    };
    for (index, child) in children.iter().enumerate() {
        prefix.push(index);
        catalogue_marked(source, child, depth + 1, budget, prefix, out);
        prefix.pop();
    }
}

/// Builds the stored ref the duplicate-pair fast-path tests share, differing
/// only in the path and bounds hash under test.
fn duplicate_entry(
    window_handle: isize,
    role: String,
    name: String,
    native_id: Option<agent_desktop_core::ElementIdentifier>,
    rect: agent_desktop_core::Rect,
    bounds_hash: Option<u64>,
    path: RefPath,
) -> RefEntry {
    let pid = agent_desktop_core::ProcessId::from(std::process::id());
    let mut entry = crate::tree::walk::walker_fake::ref_entry(&role);
    entry.process = agent_desktop_core::RefProcess {
        pid,
        process_instance: Some(
            crate::system::process_identity::token_for_pid(pid)
                .expect("the token read answers")
                .expect("a live process has a token"),
        ),
    };
    entry.identity.name = Some(name);
    entry.identity.native_id = native_id;
    entry.identity.basis = Some(agent_desktop_core::IdentityBasis::NativeId);
    entry.geometry = agent_desktop_core::RefGeometry {
        bounds_space: None,
        bounds: Some(rect),
        bounds_hash,
    };
    entry.source.source_window_id = Some(format!("w-{window_handle}"));
    entry.source.source_window_title = Some(TITLE_MARKER.into());
    entry.scope.path = path;
    entry
}

/// The path tier's own refutation: a landing on a sibling the identity evidence refutes is refused
/// by the bounds hash alone, while an identity ref settles ambiguity over the whole scope.
#[test]
fn the_fast_path_refuses_a_contradicting_hash_and_the_identity_ref_stays_ambiguous() {
    crate::tree::fixture::ensure_test_apartment();
    let hosted =
        DuplicatePairWindow::create(PairGeometry::Separated).expect("a duplicate window hosts");
    let deadline = crate::tree::walk::walker_fake::deadline();
    let root = crate::tree::automation::root_from_hwnd(hosted.handle, deadline)
        .expect("the hosted window resolves");
    let source = UiaTreeSource::for_root(&root).expect("a tree source");
    let prepared = source.prepare_root(&root).expect("a prepared root");
    let budget = WalkBudget::new(10, deadline);

    let mut prefix = Vec::new();
    let mut catalogued = Vec::new();
    catalogue_marked(&source, &prepared, 0, &budget, &mut prefix, &mut catalogued);
    assert_eq!(
        catalogued.len(),
        2,
        "the host must present exactly two marked controls"
    );
    let (_, stored) = &catalogued[0];
    let (impostor_path, impostor) = &catalogued[1];
    let role = stored.role.known().cloned().expect("a read role");
    let name = stored.name.known().cloned().expect("a read name");
    let native_id = automation_id(stored);
    assert_eq!(
        automation_id(impostor),
        native_id,
        "the pair must share identity, or the contradiction has nothing to disprove"
    );

    let stored_hash = crate::tree::resolve::resolve_match::bounds_hash_of(stored)
        .expect("the stored candidate has a positive-area bounds hash");
    let impostor_hash = crate::tree::resolve::resolve_match::bounds_hash_of(impostor)
        .expect("the impostor candidate has a positive-area bounds hash");
    assert_ne!(
        stored_hash, impostor_hash,
        "the two controls must occupy distinct rectangles, or the fast path has nothing to refute"
    );
    let rect = *stored
        .ref_evidence
        .bounds
        .known()
        .expect("a read bounds rectangle");

    let contradicting = duplicate_entry(
        hosted.handle,
        role.clone(),
        name.clone(),
        native_id.clone(),
        rect,
        Some(stored_hash),
        impostor_path.clone(),
    );
    let landed_on_impostor =
        walk_stored_path(&source, &prepared, &contradicting.scope.path, &budget)
            .expect("the path walk answers")
            .element
            .expect("the stored path still lands on a live element");
    assert!(
        accept_path_landing(&source, &landed_on_impostor, &contradicting).is_none(),
        "a live bounds hash that contradicts the stored one must not be accepted by the fast path"
    );

    let error = resolve_element_strict(&contradicting, deadline)
        .err()
        .expect("the scope holds two elements that answer to the stored identity");
    assert_eq!(
        error.code,
        ErrorCode::AmbiguousTarget,
        "the stored hash must not choose between two identity matches: {error:?}"
    );

    let equal_hash_entry = duplicate_entry(
        hosted.handle,
        role,
        name,
        native_id,
        rect,
        Some(impostor_hash),
        impostor_path.clone(),
    );
    assert!(
        accept_path_landing(&source, &landed_on_impostor, &equal_hash_entry).is_some(),
        "equal hashes must not be refused"
    );
}

/// The guard against the fix over-tightening: an unread live bounds hash is
/// never a refutation. Pinned directly over evidence, no live window needed,
/// because a live UI Automation button never fails a bounds read - the
/// tri-state discipline this predicate honours exists for providers that do,
/// the same discipline `resolve_search_admission_tests.rs` pins for
/// `admit_node`. Without this direction, treating any live/stored
/// disagreement as a refutation - including an unread one - would strand
/// every ref whose bounds momentarily failed to read.
#[test]
fn a_path_landing_with_an_unreadable_live_bounds_hash_is_still_accepted() {
    let mut entry = crate::tree::walk::walker_fake::ref_entry("button");
    entry.identity.basis = Some(agent_desktop_core::IdentityBasis::Name);
    entry.identity.name = Some("Save".into());
    entry.geometry.bounds_hash = Some(0xdead_beef);
    entry.scope.path_is_absolute = true;
    let unreadable = LocatorEvidence {
        role: LocatorField::Known("button".into()),
        name: LocatorField::Known("Save".into()),
        value: LocatorField::Absent,
        description: LocatorField::Absent,
        identifiers: agent_desktop_core::IdentifierEvidence::absent(),
        states: LocatorField::Absent,
        ref_evidence: agent_desktop_core::LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Unknown,
            available_actions: LocatorField::Absent,
            descriptors: agent_desktop_core::NodeDescriptor::default(),
        },
    };

    assert!(
        !geometry_contradicts(&entry, &unreadable),
        "an unreadable live bounds hash must never contradict the stored one"
    );
    assert_eq!(
        admit_node(&entry, &unreadable),
        NodeAdmission::Collect,
        "the identity tiers alone still settle the match, so the landing is still collected"
    );
}
