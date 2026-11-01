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

#[test]
fn a_replacement_holding_the_same_path_but_another_retained_object_is_refuted() {
    let mut observed = node(0, evidence("button", Some("ok")), vec![], &[]);
    observed.evidence.identifiers =
        crate::IdentifierEvidence::absent().with_retained_object("A".into());
    let tree = tree(vec![observed.clone()], vec![0], true);
    let entry =
        super::super::materialize::ref_entry(&observed, &tree.source, &LocatorQuery::default());
    assert_eq!(anchor_matches(&entry, &observed), IdentityMatch::Match);

    let mut replacement = observed.clone();
    replacement.evidence.identifiers =
        crate::IdentifierEvidence::absent().with_retained_object("B".into());
    assert_eq!(anchor_matches(&entry, &replacement), IdentityMatch::NoMatch);

    replacement.evidence.identifiers = crate::IdentifierEvidence::absent();
    assert_eq!(anchor_matches(&entry, &replacement), IdentityMatch::Unknown);
}
