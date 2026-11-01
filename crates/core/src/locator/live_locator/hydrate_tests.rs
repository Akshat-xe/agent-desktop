use super::*;
use crate::locator::live_locator::test_support::{evidence, node, tree};

#[test]
fn selected_hydration_preserves_window_geometry_and_refuses_unread_or_mixed_frames() {
    use crate::{BoundsSpace, LocatorField, Rect};
    let bounds = Rect {
        x: 10.0,
        y: 20.0,
        width: 80.0,
        height: 30.0,
    };
    let query = LocatorQuery::default();
    let mut observed = node(0, evidence("button", None), vec![], &[]);
    observed.evidence.ref_evidence.bounds = LocatorField::Unknown;
    observed.evidence.ref_evidence.window_bounds = LocatorField::Known(bounds);
    let source = tree(vec![observed.clone()], vec![0], true).source;
    let mut entry = super::super::materialize::ref_entry(&observed, &source, &query);
    entry.identity.basis = Some(crate::IdentityBasis::Positional);
    assert_eq!(entry.geometry.bounds_space, Some(BoundsSpace::Window));
    assert!(evidence_complete(&query, &observed, BoundsSpace::Window));
    assert_eq!(
        anchor_matches(&entry, &observed),
        IdentityMatch::Match,
        "a positional anchor is confirmed by its recorded window geometry"
    );

    observed.evidence.ref_evidence.window_bounds = LocatorField::Known(Rect { x: 110.0, ..bounds });
    observed.evidence.ref_evidence.bounds = LocatorField::Known(bounds);
    assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::NoMatch);
    observed.evidence.ref_evidence.bounds = LocatorField::Unknown;
    for window in [LocatorField::Unknown, LocatorField::Absent] {
        observed.evidence.ref_evidence.window_bounds = window;
        assert!(!evidence_complete(&query, &observed, BoundsSpace::Window));
        assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::NoMatch);
    }
    observed.evidence.ref_evidence.window_bounds = LocatorField::Unknown;
    observed.evidence.ref_evidence.bounds = LocatorField::Known(bounds);
    assert!(!evidence_complete(&query, &observed, BoundsSpace::Window));
    observed.evidence.ref_evidence.window_bounds = LocatorField::Known(bounds);
    observed.evidence.ref_evidence.bounds = LocatorField::Unknown;
    entry.geometry.bounds_space = Some(BoundsSpace::Screen);
    assert!(!evidence_complete(&query, &observed, BoundsSpace::Screen));
    assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::NoMatch);
}

#[test]
fn matching_geometry_cannot_confirm_an_anchor_that_recorded_an_identifier() {
    let mut field = evidence("button", None);
    field.identifiers = crate::IdentifierEvidence::typed(
        [crate::ElementIdentifier {
            kind: crate::IdentifierKind::AxIdentifier,
            value: "row-7".into(),
        }],
        Some(0),
        true,
    );
    let observed = node(0, field, vec![], &[]);
    let tree = tree(vec![observed.clone()], vec![0], true);
    let mut entry =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    entry.identity.basis = Some(crate::IdentityBasis::NativeId);
    assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::Match);

    let mut replacement = observed.clone();
    replacement.evidence.identifiers = crate::IdentifierEvidence::typed(
        [crate::ElementIdentifier {
            kind: crate::IdentifierKind::AxIdentifier,
            value: "row-8".into(),
        }],
        Some(0),
        true,
    );
    assert_eq!(
        anchor_matches(&entry, &replacement),
        IdentityMatch::NoMatch,
        "the same recorded geometry must not stand in for a native identifier"
    );

    replacement.evidence.identifiers = crate::IdentifierEvidence::unknown();
    assert_eq!(
        anchor_matches(&entry, &replacement),
        IdentityMatch::Unknown,
        "an incomplete identifier read can never resolve"
    );
}

#[test]
fn matching_geometry_cannot_confirm_an_anchor_that_recorded_a_name() {
    use crate::LocatorField;
    let observed = node(0, evidence("button", Some("Send")), vec![], &[]);
    let tree = tree(vec![observed.clone()], vec![0], true);
    let mut entry =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    entry.identity.basis = Some(crate::IdentityBasis::Name);
    entry.identity.name = Some("Send".into());
    assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::Match);

    let mut replacement = observed.clone();
    replacement.evidence.name = LocatorField::Known("Cancel".into());
    assert_eq!(
        anchor_matches(&entry, &replacement),
        IdentityMatch::NoMatch,
        "a name anchor is refuted by a different live name"
    );

    replacement.evidence.name = LocatorField::Unknown;
    assert_eq!(
        anchor_matches(&entry, &replacement),
        IdentityMatch::Unknown,
        "an unread live name can never resolve"
    );
}

#[test]
fn an_anchor_that_shared_its_identifier_is_refuted_by_the_name_it_recorded() {
    use crate::{ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorField};
    let shared = ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "row".into(),
    };
    let mut alpha = evidence("cell", Some("Alpha"));
    alpha.identifiers = IdentifierEvidence::typed([shared], Some(0), true);
    let observed = node(0, alpha, vec![], &[]);
    let tree = tree(vec![observed.clone()], vec![0], true);
    let mut entry =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    entry.identity.basis = Some(crate::IdentityBasis::Name);
    assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::Match);

    let mut beta = observed.clone();
    beta.evidence.name = LocatorField::Known("Beta".into());
    assert_eq!(
        anchor_matches(&entry, &beta),
        IdentityMatch::NoMatch,
        "the identifier is shared, so only the recorded name can tell Alpha from Beta"
    );
}

#[test]
fn a_reordered_row_that_shares_its_name_and_identifier_is_refuted_by_its_description() {
    use crate::{ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorField};
    let shared = ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "row".into(),
    };
    let mut stored = evidence("cell", Some("Alpha"));
    stored.description = LocatorField::Known("first row".into());
    stored.identifiers = IdentifierEvidence::typed([shared], Some(0), true);
    let observed = node(0, stored, vec![], &[]);
    let tree = tree(vec![observed.clone()], vec![0], true);
    let mut entry =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    assert_eq!(entry.identity.description.as_deref(), Some("first row"));

    let mut reordered = observed.clone();
    reordered.evidence.description = LocatorField::Known("second row".into());
    for basis in [
        crate::IdentityBasis::NativeId,
        crate::IdentityBasis::Name,
        crate::IdentityBasis::Description,
        crate::IdentityBasis::Positional,
    ] {
        entry.identity.basis = Some(basis);
        assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::Match);
        assert_eq!(
            anchor_matches(&entry, &reordered),
            IdentityMatch::NoMatch,
            "{basis:?}: the shared name and identifier cannot stand in for the description the anchor recorded"
        );
        reordered.evidence.description = LocatorField::Unknown;
        assert_eq!(
            anchor_matches(&entry, &reordered),
            IdentityMatch::Unknown,
            "{basis:?}: an unread live description can never resolve the anchor"
        );
        reordered.evidence.description = LocatorField::Known("second row".into());
    }
}

#[test]
fn a_hydrated_match_keeps_the_basis_its_survey_proved() {
    let observed = node(0, evidence("button", Some("Send")), vec![], &[]);
    let tree = tree(vec![observed.clone()], vec![0], true);
    let mut preliminary =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    preliminary.identity.basis = Some(crate::IdentityBasis::NativeId);
    let mut hydrated =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    assert_eq!(
        hydrated.identity.basis,
        Some(crate::IdentityBasis::Ambiguous)
    );
    preserve_verified_identity(&preliminary, &observed, &mut hydrated);
    assert_eq!(
        hydrated.identity.basis,
        Some(crate::IdentityBasis::NativeId)
    );
}

#[test]
fn a_name_anchor_is_refuted_by_a_look_alike_in_another_identifier_group() {
    use crate::{ElementIdentifier, IdentifierEvidence, IdentifierKind, LocatorField};
    let recorded = ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "row-1".into(),
    };
    let mut observed = node(0, evidence("cell", Some("Investors")), vec![], &[]);
    observed.evidence.identifiers = IdentifierEvidence::typed([recorded], Some(0), true);
    let tree = tree(vec![observed.clone()], vec![0], true);
    let mut entry =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    entry.identity.basis = Some(crate::IdentityBasis::Name);
    assert_eq!(entry.identity.name.as_deref(), Some("Investors"));
    assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::Match);

    let mut look_alike = observed.clone();
    look_alike.evidence.identifiers = IdentifierEvidence::typed(
        [ElementIdentifier {
            kind: IdentifierKind::AxIdentifier,
            value: "row-2".into(),
        }],
        Some(0),
        true,
    );
    assert_eq!(
        anchor_matches(&entry, &look_alike),
        IdentityMatch::NoMatch,
        "identical text under another identifier is not this anchor's element"
    );

    look_alike.evidence.identifiers = IdentifierEvidence::unknown();
    assert_eq!(
        anchor_matches(&entry, &look_alike),
        IdentityMatch::Unknown,
        "an unread identifier list can never resolve"
    );

    let mut unreadable = observed.clone();
    unreadable.evidence.name = LocatorField::Unknown;
    let mut named_query = LocatorQuery::default();
    named_query.identity.name = Some("Investors".into());
    assert!(!evidence_complete(
        &named_query,
        &unreadable,
        crate::BoundsSpace::Screen
    ));
}

fn hydrated_field(role: &str, value_known: bool) -> (RefEntry, ObservedNode) {
    use crate::{ElementIdentifier, IdentifierEvidence, IdentifierKind};
    let mut observed = node(0, evidence(role, Some("draft")), vec![], &[]);
    observed.evidence.identifiers = IdentifierEvidence::typed(
        [ElementIdentifier {
            kind: IdentifierKind::AxIdentifier,
            value: "field-1".into(),
        }],
        Some(0),
        true,
    );
    observed.evidence.value = if value_known {
        crate::LocatorField::Known("other".into())
    } else {
        crate::LocatorField::Unknown
    };
    let tree = tree(vec![observed.clone()], vec![0], true);
    let mut preliminary =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    preliminary.identity.basis = Some(crate::IdentityBasis::Positional);
    let mut hydrated = preliminary.clone();
    preserve_verified_identity(&preliminary, &observed, &mut hydrated);
    (hydrated, observed)
}

#[test]
fn an_unverified_mutable_name_is_not_saved_and_a_mirrored_read_still_resolves() {
    let (entry, observed) = hydrated_field("textfield", false);
    assert_eq!(entry.identity.name, None);
    let mut later = observed.clone();
    later.evidence.value = crate::LocatorField::Known("draft".into());
    assert_eq!(anchor_matches(&entry, &later), IdentityMatch::Match);
}

#[test]
fn a_mutable_name_confirmed_by_a_read_value_is_kept() {
    let (entry, _) = hydrated_field("textfield", true);
    assert_eq!(entry.identity.name.as_deref(), Some("draft"));
    let (button, _) = hydrated_field("button", false);
    assert_eq!(button.identity.name.as_deref(), Some("draft"));
}
