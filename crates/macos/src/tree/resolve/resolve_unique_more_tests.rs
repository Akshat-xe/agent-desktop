use super::tests::{evidence, identity_entry};
use super::*;
use agent_desktop_core::{
    ElementIdentifier, ErrorCode, IdentifierEvidence, IdentifierKind, IdentityBasis, LocatorField,
};

#[test]
fn an_expired_deadline_never_resolves_a_unique_ref() {
    let stored = identity_entry(IdentityBasis::Name);
    let mut context = ResolveReadContext::new(std::time::Instant::now());

    let error = find_unique_entry_in_roots(
        &[AXElement(std::ptr::null_mut())],
        &stored,
        50,
        &mut context,
    )
    .err()
    .expect("an expired deadline must never resolve");

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn a_name_ref_neither_counts_nor_matches_a_look_alike_in_another_role() {
    let stored = identity_entry(IdentityBasis::Name);
    let same_role = evidence(
        LocatorField::Known("cell".into()),
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        IdentifierEvidence::absent(),
    );
    let other_role = evidence(
        LocatorField::Known("button".into()),
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        IdentifierEvidence::absent(),
    );
    assert_eq!(
        identity_candidate_verdict(&stored, &same_role),
        IdentityMatch::Match
    );
    assert_eq!(
        identity_candidate_verdict(&stored, &other_role),
        IdentityMatch::NoMatch,
        "identical text in another role is not this ref's element"
    );

    let mut tally = UniqueTally::default();
    tally.record(AXElement(std::ptr::null_mut()));
    assert!(
        tally.into_resolution(&stored).is_ok(),
        "a look-alike in another role must not make the count ambiguous"
    );

    let absent = UniqueTally::default().into_resolution(&stored).err();
    assert_eq!(
        absent.map(|error| error.code),
        Some(ErrorCode::ElementNotFound)
    );
}

#[test]
fn identical_text_in_another_identifier_group_is_neither_counted_nor_matched() {
    let recorded = ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "row-1".into(),
    };
    let mut stored = identity_entry(IdentityBasis::Name);
    stored.identity.native_id = Some(recorded.clone());
    let look_alike = evidence(
        LocatorField::Known("cell".into()),
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        IdentifierEvidence::typed(
            [ElementIdentifier {
                kind: IdentifierKind::AxIdentifier,
                value: "row-2".into(),
            }],
            Some(0),
            true,
        ),
    );
    let same_group = evidence(
        LocatorField::Known("cell".into()),
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        IdentifierEvidence::typed([recorded], Some(0), true),
    );
    let unreadable_group = evidence(
        LocatorField::Known("cell".into()),
        LocatorField::Known("Investors".into()),
        LocatorField::Absent,
        IdentifierEvidence::unknown(),
    );

    assert_eq!(
        identity_candidate_verdict(&stored, &look_alike),
        IdentityMatch::NoMatch,
        "identical text under a different identifier is not this ref's element"
    );
    assert_eq!(
        identity_candidate_verdict(&stored, &same_group),
        IdentityMatch::Match
    );
    assert_eq!(
        identity_candidate_verdict(&stored, &unreadable_group),
        IdentityMatch::Unknown,
        "an unread identifier list can never prove a non-match"
    );

    let mut tally = UniqueTally::default();
    tally.record(AXElement(std::ptr::null_mut()));
    assert!(
        tally.into_resolution(&stored).is_ok(),
        "a look-alike in another identifier group must not make the count ambiguous"
    );
}
