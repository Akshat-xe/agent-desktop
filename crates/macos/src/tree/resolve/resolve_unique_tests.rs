use super::*;
use crate::tree::resolve::resolve_errors::incomplete_traversal;
use crate::tree::resolve::tests::entry;
use agent_desktop_core::{
    ElementIdentifier, ErrorCode, IdentifierEvidence, IdentifierKind, IdentityBasis, LocatorField,
    LocatorRefEvidence,
};

fn identity_entry(basis: IdentityBasis) -> RefEntry {
    let mut stored = entry(Some(42), Some("w-42"), None, None);
    stored.identity.basis = Some(basis);
    stored
}

fn evidence(
    role: LocatorField<String>,
    name: LocatorField<String>,
    description: LocatorField<String>,
    identifiers: IdentifierEvidence,
) -> LocatorEvidence {
    LocatorEvidence {
        role,
        name,
        description,
        value: LocatorField::Absent,
        identifiers,
        states: LocatorField::Absent,
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Absent,
            descriptors: Default::default(),
        },
    }
}

fn live_element() -> AXElement {
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    AXElement(unsafe { accessibility_sys::AXUIElementCreateApplication(pid) })
}

#[test]
fn a_recorded_bounds_hash_never_breaks_a_duplicate_identity() {
    let stored = identity_entry(IdentityBasis::NativeId);
    assert_eq!(stored.geometry.bounds_hash, Some(42));
    let mut tally = UniqueTally::default();
    tally.record(AXElement(std::ptr::null_mut()));
    tally.record(AXElement(std::ptr::null_mut()));

    let error = tally
        .into_resolution(&stored)
        .err()
        .expect("a duplicate identity must never resolve");

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(error.details.unwrap()["candidate_count"], 2);
}

#[test]
fn one_match_in_an_unfinished_search_never_resolves() {
    let stored = identity_entry(IdentityBasis::NativeId);
    let mut tally = UniqueTally::default();
    tally.record(AXElement(std::ptr::null_mut()));
    tally.mark_incomplete(incomplete_traversal("children", 2));

    let error = tally
        .into_resolution(&stored)
        .err()
        .expect("one match in an unfinished search must never resolve");

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(
        error.details.unwrap()["kind"],
        "resolution_traversal_incomplete"
    );
}

#[test]
fn one_match_in_a_complete_search_resolves() {
    let stored = identity_entry(IdentityBasis::Name);
    let mut tally = UniqueTally::default();
    tally.record(AXElement(std::ptr::null_mut()));

    assert!(tally.into_resolution(&stored).is_ok());
}

#[test]
fn a_complete_search_without_a_match_is_absent() {
    let stored = identity_entry(IdentityBasis::Description);

    let error = UniqueTally::default()
        .into_resolution(&stored)
        .err()
        .expect("an absent identity must never resolve");

    assert_eq!(error.code, ErrorCode::ElementNotFound);
}

#[test]
fn a_proven_duplicate_never_resolves_even_when_the_search_was_cut_short() {
    let stored = identity_entry(IdentityBasis::NativeId);
    let mut tally = UniqueTally::default();
    tally.record(AXElement(std::ptr::null_mut()));
    tally.record(AXElement(std::ptr::null_mut()));
    tally.mark_incomplete(incomplete_traversal("depth_limit", 50));

    let error = tally
        .into_resolution(&stored)
        .err()
        .expect("a duplicate identity must never resolve");

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn one_live_element_visited_twice_is_one_candidate() {
    let stored = identity_entry(IdentityBasis::Name);
    let element = live_element();
    let mut tally = UniqueTally::default();
    tally.record(element.clone());
    tally.record(element);

    assert!(tally.into_resolution(&stored).is_ok());
}

#[test]
fn a_candidate_with_the_right_identifier_on_another_role_never_counts() {
    let identifier = ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "compose".into(),
    };
    let mut stored = identity_entry(IdentityBasis::NativeId);
    stored.identity.native_id = Some(identifier.clone());
    let identifiers = IdentifierEvidence::typed([identifier], Some(0), true);

    let other_role = evidence(
        LocatorField::Known("button".into()),
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        identifiers.clone(),
    );
    assert_eq!(
        identity_candidate_verdict(&stored, &other_role),
        IdentityMatch::NoMatch
    );

    let same_role = evidence(
        LocatorField::Known("cell".into()),
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        identifiers,
    );
    assert_eq!(
        identity_candidate_verdict(&stored, &same_role),
        IdentityMatch::Match
    );
}

#[test]
fn an_unreadable_role_is_an_unfinished_count_rather_than_a_match() {
    let stored = identity_entry(IdentityBasis::Name);
    let unreadable = evidence(
        LocatorField::Unknown,
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        IdentifierEvidence::absent(),
    );

    assert_eq!(
        identity_candidate_verdict(&stored, &unreadable),
        IdentityMatch::Unknown
    );
}

#[test]
fn text_identity_counts_only_the_recorded_name() {
    let stored = identity_entry(IdentityBasis::Name);
    let rename = evidence(
        LocatorField::Known("cell".into()),
        LocatorField::Known("Buyers".into()),
        LocatorField::Absent,
        IdentifierEvidence::absent(),
    );

    assert_eq!(
        identity_candidate_verdict(&stored, &rename),
        IdentityMatch::NoMatch
    );
}

#[test]
fn a_duplicate_is_never_reported_as_a_complete_single_match() {
    assert_eq!(classify_unique_matches(2, true), UniqueOutcome::Duplicate);
    assert_eq!(classify_unique_matches(1, false), UniqueOutcome::Incomplete);
    assert_eq!(classify_unique_matches(1, true), UniqueOutcome::Resolved);
    assert_eq!(classify_unique_matches(0, true), UniqueOutcome::Absent);
    assert_eq!(classify_unique_matches(0, false), UniqueOutcome::Incomplete);
}
