//! The identity-uniqueness tier: what a live count decides, which candidates it
//! counts, and the two cut regions a count must never resolve past.

use super::*;
use crate::tree::resolve::resolve_search::admit_node;
use crate::tree::walk::walker_fake::FakeTree;
use agent_desktop_core::{
    ElementIdentifier, IdentifierEvidence, IdentifierKind, IdentityBasis, LocatorField,
    NodeDescriptor, Rect,
};

fn field(value: Option<&str>) -> LocatorField<String> {
    match value {
        Some(value) => LocatorField::Known(value.to_string()),
        None => LocatorField::Absent,
    }
}

fn live(
    role: &str,
    name: Option<&str>,
    value: Option<&str>,
    description: Option<&str>,
    native_id: Option<&str>,
) -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known(role.to_string()),
        name: field(name),
        value: field(value),
        description: field(description),
        identifiers: IdentifierEvidence::typed(
            native_id.into_iter().map(|value| ElementIdentifier {
                kind: IdentifierKind::AutomationId,
                value: value.to_string(),
            }),
            None,
            true,
        ),
        states: LocatorField::Absent,
        ref_evidence: agent_desktop_core::LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Absent,
            descriptors: NodeDescriptor::default(),
        },
    }
}

fn stored(
    role: &str,
    basis: IdentityBasis,
    name: Option<&str>,
    native_id: Option<&str>,
) -> RefEntry {
    let mut entry = crate::tree::walk::walker_fake::ref_entry(role);
    entry.identity.basis = Some(basis);
    entry.identity.name = name.map(str::to_string);
    entry.identity.native_id = native_id.map(|value| ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: value.to_string(),
    });
    entry
}

/// The positive half of the count: one match and nothing left unread is the
/// only shape that resolves. The index is asserted so an arm that resolved a
/// different candidate would fail rather than pass by `Resolved` alone.
#[test]
fn a_single_match_in_a_fully_read_scope_resolves() {
    assert!(matches!(
        classify_identity_search(1, false),
        SearchVerdict::Resolved(0)
    ));
}

/// A sole match found while any part of the scope stayed unread never resolves:
/// the second live element the identity would also match could be sitting in
/// the region the walk could not read, and resolving is exactly the
/// silent-wrong-target shape this tier exists to refuse.
#[test]
fn a_single_match_with_an_unread_region_is_incomplete_not_resolved() {
    assert!(matches!(
        classify_identity_search(1, true),
        SearchVerdict::Incomplete
    ));
}

/// An empty count only settles when every region was read. Otherwise the
/// element may still be there, and `STALE_REF` - "the element is gone" - would
/// be a verdict off a walk that never finished.
#[test]
fn an_empty_count_with_an_unread_region_is_incomplete_not_stale() {
    assert!(matches!(
        classify_identity_search(0, true),
        SearchVerdict::Incomplete
    ));
    assert!(matches!(
        classify_identity_search(0, false),
        SearchVerdict::Stale
    ));
}

/// Two promises cannot both be the answer, and nothing left unread could make
/// them fewer, so ambiguity is conclusive even on a partial walk - the same
/// ordering the positional classification applies.
#[test]
fn two_or_more_matches_are_ambiguous_even_on_a_partial_walk() {
    assert!(matches!(
        classify_identity_search(2, false),
        SearchVerdict::Ambiguous
    ));
    assert!(matches!(
        classify_identity_search(3, true),
        SearchVerdict::Ambiguous
    ));
}

/// The identity rule keeps the AutomationId-first corroboration: an identifier
/// that still matches while the stored stable text drifted is refuted, never
/// counted. Dropping the corroboration would count the look-alike and turn a
/// resolvable ref into `AMBIGUOUS_TARGET` - or, worse, resolve onto it.
#[test]
fn a_matching_identifier_with_a_drifted_name_is_refuted_not_counted() {
    let entry = stored(
        "row",
        IdentityBasis::NativeId,
        Some("Alpha"),
        Some("row-77"),
    );

    assert_eq!(
        admit_identity_candidate(
            &entry,
            &live("row", Some("Alpha"), None, None, Some("row-77"))
        ),
        NodeAdmission::Collect
    );
    assert_eq!(
        admit_identity_candidate(
            &entry,
            &live("row", Some("Beta"), None, None, Some("row-77"))
        ),
        NodeAdmission::Reject
    );
}

/// The role gate, both directions, plus the unreadable case: a role that read
/// back as something else is a refusal, while a role that could not be read at
/// all leaves the region unread rather than settling the count.
#[test]
fn the_role_gate_rejects_a_different_role_and_withholds_an_unreadable_one() {
    let entry = stored("button", IdentityBasis::Name, Some("Save"), None);

    assert_eq!(
        admit_identity_candidate(&entry, &live("textfield", Some("Save"), None, None, None)),
        NodeAdmission::Reject
    );
    let unreadable = LocatorEvidence {
        role: LocatorField::Unknown,
        ..live("button", Some("Save"), None, None, None)
    };
    assert_eq!(
        admit_identity_candidate(&entry, &unreadable),
        NodeAdmission::Unread
    );
}

/// A candidate whose decisive field could not be read is unread, never a
/// refusal: collapsing `Unknown` into `NoMatch` would let a one-candidate count
/// settle off a provider that merely failed to answer.
#[test]
fn an_unreadable_identifier_is_unread_not_refuted() {
    let entry = stored("row", IdentityBasis::NativeId, None, Some("row-77"));
    let unreadable = LocatorEvidence {
        identifiers: IdentifierEvidence::unknown(),
        ..live("row", None, None, None, None)
    };

    assert_eq!(
        admit_identity_candidate(&entry, &unreadable),
        NodeAdmission::Unread
    );
}

/// Bounds never confirm an identity ref. The one candidate below is the shape
/// the positional search's geometry promotion exists for - an unsettleable
/// identity plus an agreeing positive-area rectangle - and that search does
/// collect it. The identity tier counts nothing there, so a stored rectangle
/// can never stand in for the identity it did not record.
#[test]
fn geometry_that_promotes_a_positional_candidate_confirms_nothing_here() {
    let bounds = Rect {
        x: 12.0,
        y: 24.0,
        width: 40.0,
        height: 20.0,
    };
    let mut entry = stored("textfield", IdentityBasis::Positional, None, None);
    entry.geometry = agent_desktop_core::RefGeometry {
        bounds_space: None,
        bounds: Some(bounds),
        bounds_hash: bounds.bounds_hash(),
    };
    let evidence = LocatorEvidence {
        role: LocatorField::Known("textfield".to_string()),
        name: LocatorField::Absent,
        value: LocatorField::Absent,
        description: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Absent,
        ref_evidence: agent_desktop_core::LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Known(bounds),
            available_actions: LocatorField::Absent,
            descriptors: NodeDescriptor::default(),
        },
    };

    assert_eq!(
        admit_node(&entry, &evidence),
        NodeAdmission::Collect,
        "the positional search promotes on agreeing geometry, so this fixture has teeth"
    );
    assert_eq!(
        admit_identity_candidate(&entry, &evidence),
        NodeAdmission::Unread,
        "an identity ref's stored rectangle must never confirm a candidate"
    );
}

/// A ref recorded before identity evidence was kept is refused from its own
/// record, before any window resolution or search. Fails if the gate is moved
/// behind either: the refusal would then carry the search's own diagnosis, and
/// a legacy ref would pay for a full walk on every attempt.
#[test]
fn a_legacy_entry_is_refused_before_any_live_work() {
    let mut entry = crate::tree::walk::walker_fake::ref_entry("button");
    entry.source.source_window_id = Some("w-not-a-real-window".into());

    let error =
        crate::tree::resolve::resolve_attempt(&entry, crate::tree::walk::walker_fake::deadline())
            .err()
            .expect("a ref with no recorded basis must never resolve");

    assert_eq!(error.code, agent_desktop_core::ErrorCode::StaleRef);
    assert_eq!(
        error.message, "The saved ref was recorded before identity evidence was kept",
        "the refusal must name the missing basis, not a live mismatch"
    );
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

/// A ref the snapshot could not prove unique is refused the same way, with the
/// identity-not-unique diagnosis and a not-delivered disposition: nothing was
/// sent to the application, and one look-alike's action must never be run
/// against the other.
#[test]
fn an_ambiguous_entry_is_refused_with_the_identity_not_unique_diagnosis() {
    let mut entry = crate::tree::walk::walker_fake::ref_entry("row");
    entry.identity.basis = Some(IdentityBasis::Ambiguous);
    entry.identity.name = Some("Alpha".into());

    let error =
        crate::tree::resolve::resolve_attempt(&entry, crate::tree::walk::walker_fake::deadline())
            .err()
            .expect("an ambiguous ref must never resolve");

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

fn children_budget(max_siblings: usize) -> WalkBudget {
    crate::tree::walk::walker_fake::budget(10).with_max_siblings(max_siblings)
}

/// A list that reached the sibling cap with more siblings behind it is a cut,
/// not an end: a look-alike can sit in the part the enumeration never returned.
/// The probe is the only signal that separates this from a list that genuinely
/// ends at the cap.
#[test]
fn a_sibling_list_that_reached_the_cap_with_more_behind_it_is_cut() {
    let tree = FakeTree::default().with_children(1, &[2, 3, 4]);

    assert!(
        super::sibling_list_was_cut(&tree, &[2, 3], &children_budget(2)),
        "a sibling past the cap means the list the search saw is not the whole one"
    );
    assert!(
        !super::sibling_list_was_cut(&tree, &[2, 3], &children_budget(4)),
        "a list shorter than the cap was read to its end"
    );
}

/// The other direction on a real end: the last returned sibling has none, so a
/// list that happens to reach the cap is whole and must not be withheld.
#[test]
fn a_sibling_list_that_ends_at_the_cap_is_not_cut() {
    let tree = FakeTree::default().with_children(1, &[2, 3]);

    assert!(!super::sibling_list_was_cut(
        &tree,
        &[2, 3],
        &children_budget(2)
    ));
}

/// A probe that fails is unreadable, not empty: the search cannot claim the
/// list ended, so the count it produced is withheld.
#[test]
fn a_failing_sibling_probe_is_a_cut_not_an_end() {
    let tree = FakeTree::default()
        .with_children(1, &[2, 3, 4])
        .faulting_on_next_sibling(3);

    assert!(super::sibling_list_was_cut(
        &tree,
        &[2, 3],
        &children_budget(2)
    ));
}

/// The depth budget cuts depth-first, so a node at the cap with children is a
/// region the count never read. A leaf there is a genuine end.
#[test]
fn a_node_at_the_depth_cap_cuts_the_count_only_when_it_has_children() {
    let parent = FakeTree::default().with_children(1, &[2]);
    assert!(super::boundary_hides_descendants(&parent, &1));
    assert!(!super::boundary_hides_descendants(&parent, &2));
}

/// A failed child read at the cap is not proof of a leaf: an unreadable
/// subtree is exactly what must withhold the count.
#[test]
fn an_unreadable_child_at_the_depth_cap_is_a_cut() {
    let unreadable = FakeTree::default().faulting_on_first_child(1);

    assert!(super::boundary_hides_descendants(&unreadable, &1));
}
