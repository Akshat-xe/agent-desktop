//! What the live uniqueness count counts, and every region that leaves it incomplete: a second
//! look-alike the walk never reached must withhold the answer, never become a single match.

use std::time::Duration;

use super::*;
use crate::tree::properties::PropertyOutcome;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::resolve::resolve_search::{
    SEARCH_DESCENT, descent::read_children, entry_is_unverifiable, provisional_geometry_candidate,
};
use crate::tree::resolve::{resolve_attempt, retry_incomplete_until};
use crate::tree::test_support::{number, text};
use crate::tree::walk::walker_fake::{FakeTree, budget, deadline, ref_entry};
use agent_desktop_core::{ElementIdentifier, IdentifierKind, IdentityBasis, LocatorEvidence, Rect};

const BUTTON: i32 = 50000;

const TARGET_NAME: &str = "Save";

fn button(tree: FakeTree, node: i32, name: &str) -> FakeTree {
    tree.reading(
        node,
        &[
            number(TreeProperty::ControlType, BUTTON),
            (TreeProperty::Name, text(name)),
        ],
    )
}

fn identified_button(tree: FakeTree, node: i32, name: &str, automation_id: &str) -> FakeTree {
    tree.reading(
        node,
        &[
            number(TreeProperty::ControlType, BUTTON),
            (TreeProperty::Name, text(name)),
            (TreeProperty::AutomationId, text(automation_id)),
        ],
    )
}

/// A control whose provider answered that it has no identifier at all, which the count can
/// exclude; a read that never answered is `Unknown` and cannot be excluded.
fn button_without_an_identifier(tree: FakeTree, node: i32, name: &str) -> FakeTree {
    tree.reading(
        node,
        &[
            number(TreeProperty::ControlType, BUTTON),
            (TreeProperty::Name, text(name)),
            (TreeProperty::AutomationId, PropertyOutcome::Absent),
        ],
    )
}

fn evidence_at(tree: &FakeTree, node: &i32) -> LocatorEvidence {
    tree.evidence(node).1
}

fn name_entry(role: &str) -> RefEntry {
    let mut entry = ref_entry(role);
    entry.identity.basis = Some(IdentityBasis::Name);
    entry.identity.name = Some(TARGET_NAME.to_string());
    entry
}

/// The stored ref a cap-free snapshot would issue for `node`, its role read off the fixture so
/// the role gate never decides these cases.
fn matching_entry(tree: &FakeTree, node: &i32) -> RefEntry {
    let role = evidence_at(tree, node)
        .role
        .known()
        .cloned()
        .expect("the fixture node publishes a role");
    name_entry(&role)
}

/// The sibling cap is the node limit the count walks under, so a second match the enumeration never
/// returned is visible only to a probe past the last element.
#[test]
fn a_second_match_past_the_sibling_cap_withholds_the_count() {
    let tree = FakeTree::default().with_children(1, &[2, 3, 4]);
    let tree = button(tree, 2, TARGET_NAME);
    let tree = button(tree, 4, TARGET_NAME);
    let capped = budget(10).with_max_siblings(2);

    let entry = matching_entry(&tree, &2);
    assert_eq!(
        admit_identity_candidate(&entry, &evidence_at(&tree, &2)),
        NodeAdmission::Collect
    );
    assert_eq!(
        admit_identity_candidate(&entry, &evidence_at(&tree, &4)),
        NodeAdmission::Collect,
        "the element the cap keeps out answers to the stored identity"
    );

    let read = read_children(&tree, &1, &capped, &SEARCH_DESCENT, None)
        .expect("the search enumeration answers without surfacing");
    assert_eq!(read.elements, vec![2, 3]);
    assert!(
        read.complete,
        "a cap-hit list is reported whole, which is why the probe below exists"
    );
    let cut = sibling_list_was_cut(&tree, &read.elements, &capped);
    assert!(
        cut,
        "a sibling past the cap means the count did not see the whole list"
    );

    assert!(
        matches!(classify_identity_search(1, cut), SearchVerdict::Incomplete),
        "one match with a look-alike past the cap is not a resolved count"
    );
}

/// The depth cap is the other node limit: a match below the deepest searched level is never
/// counted, so a sole match above it is withheld. The fixture root is depth 0.
#[test]
fn a_second_match_beyond_the_depth_cap_withholds_the_count() {
    let last = i32::from(MAX_RESOLVE_DEPTH) + 1;
    let spine: Vec<i32> = (1..=last).collect();
    let tree = FakeTree::default()
        .with_chain(&spine)
        .with_children(last, &[last + 1]);
    let tree = button(tree, 5, TARGET_NAME);
    let tree = button(tree, last + 1, TARGET_NAME);

    let entry = matching_entry(&tree, &5);
    assert_eq!(
        admit_identity_candidate(&entry, &evidence_at(&tree, &(last + 1))),
        NodeAdmission::Collect,
        "the element below the cap answers to the stored identity"
    );
    let cut = boundary_hides_descendants(&tree, &last);
    assert!(
        cut,
        "the node at the cap still has a child, so the depth budget cut the count"
    );

    assert!(
        matches!(classify_identity_search(1, cut), SearchVerdict::Incomplete),
        "one match above the cap is not a resolved count while the cap hid a subtree"
    );
}

/// A read cut by the deadline is an unfinished count, not a count of one, even with a first
/// match already found.
#[test]
fn a_read_that_times_out_after_the_first_match_withholds_the_count() {
    let tree = FakeTree::default()
        .with_children(1, &[2, 3])
        .with_children(2, &[4]);
    let tree = button(tree, 2, TARGET_NAME);
    let entry = matching_entry(&tree, &2);
    assert_eq!(
        admit_identity_candidate(&entry, &evidence_at(&tree, &2)),
        NodeAdmission::Collect,
        "the count holds one match before the read times out"
    );

    let timed_out = WalkBudget::new(10, deadline().capped(Duration::ZERO));
    assert!(
        timed_out.deadline.is_expired(),
        "a zero-length cap must produce an already-expired deadline"
    );
    let read = read_children(&tree, &2, &timed_out, &SEARCH_DESCENT, None)
        .expect("the search keeps its partial list rather than surfacing the timeout");
    assert!(
        !read.complete,
        "an enumeration cut by the deadline is not a whole list"
    );

    assert!(
        matches!(
            classify_identity_search(1, !read.complete),
            SearchVerdict::Incomplete
        ),
        "a sole match found before the timeout never resolves"
    );
}

/// A region whose needed fields cannot be read is unread, not empty: the search cannot tell a
/// look-alike from a node it merely failed to read.
#[test]
fn an_unreadable_region_after_the_first_match_withholds_the_count() {
    let tree = FakeTree::default().with_children(1, &[2, 3]);
    let tree = button(tree, 2, TARGET_NAME);
    let tree = tree.reading(
        3,
        &[
            (TreeProperty::ControlType, PropertyOutcome::Unknown),
            (TreeProperty::Name, PropertyOutcome::Unknown),
        ],
    );
    let entry = matching_entry(&tree, &2);
    assert_eq!(
        admit_identity_candidate(&entry, &evidence_at(&tree, &2)),
        NodeAdmission::Collect
    );

    let unreadable = evidence_at(&tree, &3);
    assert!(
        unreadable.role.is_unknown(),
        "the fixture node must answer nothing for its role, or this case is not the unread one"
    );
    assert_eq!(
        admit_identity_candidate(&entry, &unreadable),
        NodeAdmission::Unread,
        "an unreadable field is never a refusal, so the count cannot exclude it"
    );

    assert!(
        matches!(classify_identity_search(1, true), SearchVerdict::Incomplete),
        "one match with an unreadable region behind it never resolves"
    );
}

/// A name-basis ref recorded with an identifier was proven unique only inside that identifier
/// group, so an element outside it is neither counted nor matched.
#[test]
fn a_name_basis_ref_neither_counts_nor_matches_a_look_alike_outside_its_identifier_group() {
    let tree = FakeTree::default().with_children(1, &[2, 3, 4]);
    let tree = identified_button(tree, 2, TARGET_NAME, "row-7");
    let tree = identified_button(tree, 3, TARGET_NAME, "row-8");
    let tree = button_without_an_identifier(tree, 4, TARGET_NAME);

    let mut entry = matching_entry(&tree, &2);
    entry.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: "row-7".to_string(),
    });
    let same_group = evidence_at(&tree, &2);
    let other_group = evidence_at(&tree, &3);
    let no_group = evidence_at(&tree, &4);
    assert!(
        no_group.identifiers.identifiers().is_empty() && no_group.identifiers.is_complete(),
        "the third control answers with no identifier rather than an unreadable one"
    );

    assert_eq!(
        admit_identity_candidate(&entry, &same_group),
        NodeAdmission::Collect
    );
    assert_eq!(
        admit_identity_candidate(&entry, &other_group),
        NodeAdmission::Reject,
        "same role and text, different identifier group: not the stored element"
    );
    assert_eq!(
        admit_identity_candidate(&entry, &no_group),
        NodeAdmission::Reject,
        "same role and text, no identifier at all: not the stored element"
    );

    let counted = [same_group, other_group, no_group]
        .into_iter()
        .filter(|evidence| admit_identity_candidate(&entry, evidence) == NodeAdmission::Collect)
        .count();
    assert_eq!(
        counted, 1,
        "the look-alikes are not candidates, so the count is one and not three"
    );
    assert!(
        matches!(
            classify_identity_search(counted, false),
            SearchVerdict::Resolved(0)
        ),
        "a complete count of one resolves, and the look-alikes are not it"
    );
}

/// A ref the snapshot could not prove unique is refused even when its rectangle would let the
/// geometry tier pick a candidate. Nothing is sent to the application.
#[test]
fn an_ambiguous_entry_is_refused_before_the_bounds_tier_can_pick_a_candidate() {
    let bounds = Rect {
        x: 12.0,
        y: 24.0,
        width: 40.0,
        height: 20.0,
    };
    let mut entry = ref_entry("button");
    entry.identity.basis = Some(IdentityBasis::Ambiguous);
    entry.identity.name = Some(TARGET_NAME.to_string());
    entry.geometry = agent_desktop_core::RefGeometry {
        bounds_space: None,
        bounds: Some(bounds),
        bounds_hash: bounds.bounds_hash(),
    };
    entry.source.source_window_id = Some("w-not-a-real-window".into());

    assert!(
        provisional_geometry_candidate(&entry),
        "the entry is otherwise locatable by geometry, so the gate is what keeps it from a bounds pick"
    );
    assert!(
        !entry_is_unverifiable(&entry),
        "the positional tier would search for this entry, so only the basis gate refuses it"
    );

    let mut attempts = 0;
    let error = retry_incomplete_until(deadline(), || {
        attempts += 1;
        resolve_attempt(&entry, deadline())
    })
    .err()
    .expect("an ambiguous ref must never resolve");

    assert_eq!(attempts, 1, "a refused ref is not retried");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::AmbiguousTarget);
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("kind"))
            .and_then(serde_json::Value::as_str),
        Some("identity_not_unique")
    );
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}
