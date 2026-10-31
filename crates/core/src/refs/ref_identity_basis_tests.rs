use super::*;

fn identifiers(native_id: Option<&str>) -> IdentifierEvidence {
    IdentifierEvidence::typed(
        native_id.into_iter().map(|value| crate::ElementIdentifier {
            kind: crate::IdentifierKind::AxIdentifier,
            value: value.to_string(),
        }),
        native_id.map(|_| 0),
        true,
    )
}

fn match_of(
    entry: &RefEntry,
    name: Option<&str>,
    value: Option<&str>,
    description: Option<&str>,
    native_id: Option<&str>,
) -> IdentityMatch {
    identity_match(
        entry,
        &option_field(name),
        &option_field(value),
        &option_field(description),
        &identifiers(native_id),
    )
}

#[test]
fn each_basis_resolves_only_on_the_evidence_it_recorded() {
    let mut native = entry();
    native.identity.basis = Some(crate::IdentityBasis::NativeId);
    native.identity.native_id = Some(identifier("row-7"));
    native.identity.name = Some("Alpha".into());
    assert_eq!(
        match_of(&native, Some("Alpha"), None, None, Some("row-7")),
        IdentityMatch::Match
    );
    assert_eq!(
        match_of(&native, Some("Renamed"), None, None, Some("row-7")),
        IdentityMatch::NoMatch
    );
    assert_eq!(
        match_of(&native, Some("Alpha"), None, None, Some("row-8")),
        IdentityMatch::NoMatch
    );

    let mut named = entry();
    named.identity.basis = Some(crate::IdentityBasis::Name);
    named.identity.name = Some("Alpha".into());
    assert_eq!(
        match_of(&named, Some("Alpha"), None, None, None),
        IdentityMatch::Match
    );
    assert_eq!(
        match_of(&named, Some("Beta"), None, None, None),
        IdentityMatch::NoMatch
    );

    let mut described = entry();
    described.identity.basis = Some(crate::IdentityBasis::Description);
    described.identity.description = Some("Opens the drawer".into());
    assert_eq!(
        match_of(&described, None, None, Some("Opens the drawer"), None),
        IdentityMatch::Match
    );
    assert_eq!(
        match_of(&described, None, None, Some("Closes the drawer"), None),
        IdentityMatch::NoMatch
    );

    let mut positional = entry();
    positional.identity.basis = Some(crate::IdentityBasis::Positional);
    positional.identity.value = Some("On".into());
    assert_eq!(
        match_of(&positional, None, Some("On"), None, None),
        IdentityMatch::Match
    );
    assert_eq!(
        match_of(&positional, None, Some("Off"), None, None),
        IdentityMatch::NoMatch
    );
}

#[test]
fn a_description_entry_compares_only_the_live_description() {
    let mut described = entry();
    described.identity.basis = Some(crate::IdentityBasis::Description);
    described.identity.description = Some("Billing".into());

    assert_eq!(
        match_of(&described, Some("Billing"), None, None, None),
        IdentityMatch::NoMatch,
        "another field's name is not this field's description"
    );
    assert_eq!(
        match_of(&described, None, Some("Billing"), None, None),
        IdentityMatch::NoMatch,
        "a control value never stands in for a description"
    );
    assert_eq!(
        match_of(&described, None, None, Some("Billing"), None),
        IdentityMatch::Match
    );
}

#[test]
fn a_name_entry_compares_only_the_live_name() {
    let mut named = entry();
    named.identity.basis = Some(crate::IdentityBasis::Name);
    named.identity.name = Some("Address".into());

    assert_eq!(
        match_of(&named, None, Some("Address"), None, None),
        IdentityMatch::NoMatch,
        "a control value never stands in for a name"
    );
    assert_eq!(
        match_of(&named, Some("Address"), None, None, None),
        IdentityMatch::Match
    );
    assert_eq!(
        match_of(&named, Some("Address"), Some("Billing"), None, None),
        IdentityMatch::Match,
        "a live value never refutes a name it does not mirror"
    );
}

#[test]
fn a_native_identifier_entry_also_requires_the_recorded_stable_text() {
    let mut native = entry();
    native.identity.basis = Some(crate::IdentityBasis::NativeId);
    native.identity.native_id = Some(identifier("toolbar-action"));
    native.identity.name = Some("Save".into());

    assert_eq!(
        match_of(&native, Some("Delete"), None, None, Some("toolbar-action")),
        IdentityMatch::NoMatch,
        "a control renamed under the same typed identifier is not the captured element"
    );
    assert_eq!(
        match_of(&native, None, Some("Save"), None, Some("toolbar-action")),
        IdentityMatch::NoMatch,
        "a value never stands in for the recorded name"
    );
    assert_eq!(
        match_of(&native, Some("Save"), None, None, Some("toolbar-action")),
        IdentityMatch::Match
    );
    assert_eq!(
        identity_match(
            &native,
            &LocatorField::Unknown,
            &LocatorField::Absent,
            &LocatorField::Absent,
            &identifiers(Some("toolbar-action")),
        ),
        IdentityMatch::Unknown,
        "an unreadable recorded name never confirms the identifier"
    );
}

#[test]
fn a_shared_identifier_is_separated_by_the_recorded_name() {
    let mut native = entry();
    native.identity.basis = Some(crate::IdentityBasis::NativeId);
    native.identity.native_id = Some(identifier("shared-row"));
    native.identity.name = Some("Billing".into());

    assert_eq!(
        match_of(&native, Some("Shipping"), None, None, Some("shared-row")),
        IdentityMatch::NoMatch
    );
    assert_eq!(
        match_of(&native, Some("Billing"), None, None, Some("shared-row")),
        IdentityMatch::Match
    );
}

#[test]
fn a_name_entry_also_requires_the_recorded_description() {
    let mut named = entry();
    named.identity.basis = Some(crate::IdentityBasis::Name);
    named.identity.name = Some("Address".into());
    named.identity.description = Some("Billing".into());

    assert_eq!(
        match_of(&named, Some("Address"), None, Some("Shipping"), None),
        IdentityMatch::NoMatch
    );
    assert_eq!(
        match_of(&named, Some("Address"), None, None, None),
        IdentityMatch::NoMatch,
        "a recorded description that is gone refutes the candidate"
    );
    assert_eq!(
        match_of(&named, Some("Address"), None, Some("Billing"), None),
        IdentityMatch::Match
    );
}

#[test]
fn legacy_entries_never_match_their_own_live_evidence() {
    let mut legacy = entry();
    legacy.identity.native_id = Some(identifier("row-7"));
    legacy.identity.name = Some("Alpha".into());
    legacy.identity.description = Some("Opens the drawer".into());

    assert!(!has_meaningful_identity(&legacy));
    assert_eq!(
        match_of(
            &legacy,
            Some("Alpha"),
            None,
            Some("Opens the drawer"),
            Some("row-7")
        ),
        IdentityMatch::NoMatch
    );
}

#[test]
fn ambiguous_entries_never_match_their_own_live_evidence() {
    let mut ambiguous = entry();
    ambiguous.identity.basis = Some(crate::IdentityBasis::Ambiguous);
    ambiguous.identity.native_id = Some(identifier("row-7"));
    ambiguous.identity.name = Some("Alpha".into());

    assert!(!has_meaningful_identity(&ambiguous));
    assert_eq!(
        match_of(&ambiguous, Some("Alpha"), None, None, Some("row-7")),
        IdentityMatch::NoMatch
    );
}

#[test]
fn meaningful_identity_follows_the_recorded_basis() {
    for (basis, expected) in [
        (None, false),
        (Some(crate::IdentityBasis::NativeId), true),
        (Some(crate::IdentityBasis::Name), true),
        (Some(crate::IdentityBasis::Description), true),
        (Some(crate::IdentityBasis::Ambiguous), false),
        (Some(crate::IdentityBasis::Positional), true),
    ] {
        let mut entry = entry();
        entry.identity.basis = basis;
        entry.identity.native_id = Some(identifier("row-7"));
        entry.identity.name = Some("Alpha".into());
        assert_eq!(has_meaningful_identity(&entry), expected, "{basis:?}");
    }
    let mut bare = entry();
    bare.identity.basis = Some(crate::IdentityBasis::Positional);
    bare.identity.name = None;
    bare.identity.native_id = None;
    bare.identity.description = None;
    bare.identity.value = None;
    assert!(!has_meaningful_identity(&bare));
}

#[test]
fn a_positional_anchor_with_an_identifier_is_matched_on_that_identifier() {
    let mut anchor = entry();
    anchor.identity.basis = Some(crate::IdentityBasis::Positional);
    anchor.identity.native_id = Some(identifier("click-status"));
    anchor.identity.name = None;
    assert_eq!(
        match_of(&anchor, None, None, None, Some("click-status")),
        IdentityMatch::Match
    );
    assert_eq!(
        match_of(&anchor, None, None, None, Some("other-status")),
        IdentityMatch::NoMatch
    );
}

#[test]
fn a_shared_identifier_with_a_different_name_refutes_a_name_entry() {
    let mut named = entry();
    named.identity.basis = Some(crate::IdentityBasis::Name);
    named.identity.native_id = Some(identifier("shared-row"));
    named.identity.name = Some("Alpha".into());

    assert_eq!(
        match_of(&named, Some("Birthday"), None, None, Some("shared-row")),
        IdentityMatch::NoMatch
    );
    assert_eq!(
        match_of(&named, Some("Alpha"), None, None, Some("shared-row")),
        IdentityMatch::Match
    );
}

#[test]
fn a_different_live_identifier_refutes_a_name_entry_that_recorded_one() {
    let mut named = entry();
    named.identity.basis = Some(crate::IdentityBasis::Name);
    named.identity.native_id = Some(identifier("first-instance"));
    named.identity.name = Some("Alpha".into());

    assert_eq!(
        match_of(&named, Some("Alpha"), None, None, Some("second-instance")),
        IdentityMatch::NoMatch
    );
    assert_eq!(
        match_of(&named, Some("Alpha"), None, None, None),
        IdentityMatch::NoMatch
    );
}

#[test]
fn control_values_neither_match_nor_refute_unique_bases() {
    let mut named = entry();
    named.identity.basis = Some(crate::IdentityBasis::Name);
    named.identity.name = Some("Alpha".into());
    for value in ["On", "Off", ""] {
        assert_eq!(
            match_of(&named, Some("Alpha"), Some(value), None, None),
            IdentityMatch::Match,
            "value {value:?} must not decide a name match"
        );
    }
    assert_eq!(
        match_of(&named, Some("Beta"), Some("Alpha"), None, None),
        IdentityMatch::NoMatch
    );

    let mut native = entry();
    native.identity.basis = Some(crate::IdentityBasis::NativeId);
    native.identity.native_id = Some(identifier("row-7"));
    for value in [Some("On"), Some("Off"), None] {
        assert_eq!(
            match_of(&native, None, value, None, Some("row-7")),
            IdentityMatch::Match,
            "value {value:?} must not decide a native-id match"
        );
    }
}
