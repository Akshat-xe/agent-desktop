use super::test_support::{self, evidence};
use super::{
    IdentifierEvidence, LocatorEvidence, LocatorField, LocatorMaterialization,
    LocatorResolveRequest, LocatorSelection, ObservationRequest, ObservationRoot,
    ObservationSource, ObservedTree, resolve_query,
};
use crate::{
    AdapterError, IdentityMatch, LocatorQuery,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
};

struct IdentifierSwapAdapter;

impl ObservationOps for IdentifierSwapAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        Ok(match root {
            ObservationRoot::Window(_) => surveyed_pair(),
            ObservationRoot::Element { .. } => hydrated_anchor(root),
        })
    }

    fn resolve_locator_anchor(
        &self,
        entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<crate::NativeHandle, AdapterError> {
        assert_eq!(recorded_identifier(entry), Some("unique-a"));
        Ok(crate::NativeHandle::null())
    }
}

impl ActionOps for IdentifierSwapAdapter {}
impl InputOps for IdentifierSwapAdapter {}
impl SystemOps for IdentifierSwapAdapter {}

fn recorded_identifier(entry: &crate::RefEntry) -> Option<&str> {
    entry
        .identity
        .native_id
        .as_ref()
        .map(|identifier| identifier.value.as_str())
}

fn button_evidence(identifiers: &[(crate::IdentifierKind, &str)]) -> LocatorEvidence {
    let mut evidence = evidence("button", None);
    evidence.name = LocatorField::Unknown;
    evidence.description = LocatorField::Unknown;
    evidence.value = LocatorField::Unknown;
    evidence.identifiers = IdentifierEvidence::typed(
        identifiers
            .iter()
            .map(|(kind, value)| crate::ElementIdentifier {
                kind: *kind,
                value: (*value).to_string(),
            }),
        Some(0),
        true,
    );
    evidence.states = LocatorField::Unknown;
    evidence.ref_evidence.bounds = LocatorField::Known(crate::Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    });
    evidence.ref_evidence.available_actions =
        LocatorField::Known(vec![crate::actions::capability::CLICK.into()]);
    evidence
}

fn surveyed_pair() -> ObservedTree {
    test_support::tree(
        vec![
            test_support::node(0, evidence("window", None), vec![1, 2], &[]),
            test_support::node(
                1,
                button_evidence(&[(crate::IdentifierKind::AxIdentifier, "unique-a")]),
                Vec::new(),
                &[0],
            ),
            test_support::node(
                2,
                button_evidence(&[(crate::IdentifierKind::AxDomIdentifier, "shared")]),
                Vec::new(),
                &[1],
            ),
        ],
        vec![0],
        true,
    )
}

fn hydrated_anchor(root: ObservationRoot<'_>) -> ObservedTree {
    let mut hydrated = button_evidence(&[
        (crate::IdentifierKind::AxDomIdentifier, "shared"),
        (crate::IdentifierKind::AxIdentifier, "unique-a"),
    ]);
    hydrated.states = LocatorField::Known(Vec::new());
    let mut tree = test_support::tree(
        vec![test_support::node(0, hydrated, Vec::new(), &[])],
        vec![0],
        true,
    );
    tree.source = ObservationSource::from_root(&root, crate::SnapshotSurface::Window);
    tree
}

#[test]
fn hydration_keeps_the_identifier_the_survey_proved_instead_of_a_newly_preferred_one() {
    let adapter = IdentifierSwapAdapter;
    let query = LocatorQuery {
        identity: crate::IdentityPredicate {
            role: Some("button".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline: crate::Deadline::after(5_000).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::SelectedMatches,
    };

    let resolution = resolve_query(
        &adapter,
        &query,
        ObservationRoot::Window(&test_support::window()),
        &request,
    )
    .expect("selected hydration should keep the surveyed identity");

    let entry = resolution
        .refmap
        .as_ref()
        .and_then(|refmap| refmap.get("@e1"))
        .expect("the hydrated button is ref-able");
    assert_eq!(entry.identity.basis, Some(crate::IdentityBasis::NativeId));
    assert_eq!(
        recorded_identifier(entry),
        Some("unique-a"),
        "hydration must keep the identifier the survey counted, never a newly preferred one it never counted"
    );

    let surviving = IdentifierEvidence::typed(
        [crate::ElementIdentifier {
            kind: crate::IdentifierKind::AxDomIdentifier,
            value: "shared".into(),
        }],
        Some(0),
        true,
    );
    assert_eq!(
        crate::refs::ref_identity::identity_match(
            entry,
            &LocatorField::Unknown,
            &LocatorField::Unknown,
            &LocatorField::Unknown,
            &surviving,
        ),
        IdentityMatch::NoMatch,
        "the surviving element that shares the newly preferred identifier is not this ref's element"
    );
}
