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
