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

#[test]
fn hydration_keeps_the_identifier_the_survey_proved_instead_of_a_newly_preferred_one() {
    let mut hydrated = button_evidence(&[
        (crate::IdentifierKind::AxDomIdentifier, "shared"),
        (crate::IdentifierKind::AxIdentifier, "unique-a"),
    ]);
    hydrated.states = LocatorField::Known(Vec::new());
    let adapter = RecheckAdapter {
        survey: vec![
            button_evidence(&[(crate::IdentifierKind::AxIdentifier, "unique-a")]),
            button_evidence(&[(crate::IdentifierKind::AxDomIdentifier, "shared")]),
        ],
        hydrated,
    };

    let resolution =
        resolve_selected(&adapter).expect("selected hydration should keep the surveyed identity");

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

struct RecheckAdapter {
    survey: Vec<LocatorEvidence>,
    hydrated: LocatorEvidence,
}

impl ObservationOps for RecheckAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        Ok(match root {
            ObservationRoot::Window(_) => {
                let count = self.survey.len();
                let mut nodes = vec![test_support::node(
                    0,
                    evidence("window", None),
                    (1..=count as u32).collect(),
                    &[],
                )];
                for (index, survey) in self.survey.iter().enumerate() {
                    nodes.push(test_support::node(
                        index as u32 + 1,
                        survey.clone(),
                        Vec::new(),
                        &[index],
                    ));
                }
                test_support::tree(nodes, vec![0], true)
            }
            ObservationRoot::Element { .. } => {
                let mut tree = test_support::tree(
                    vec![test_support::node(
                        0,
                        self.hydrated.clone(),
                        Vec::new(),
                        &[],
                    )],
                    vec![0],
                    true,
                );
                tree.source = ObservationSource::from_root(&root, crate::SnapshotSurface::Window);
                tree
            }
        })
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &crate::RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<crate::NativeHandle, AdapterError> {
        Ok(crate::NativeHandle::null())
    }
}

impl ActionOps for RecheckAdapter {}
impl InputOps for RecheckAdapter {}
impl SystemOps for RecheckAdapter {}

fn resolve_selected(adapter: &RecheckAdapter) -> Result<super::LocatorResolution, crate::AppError> {
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
    resolve_query(
        adapter,
        &query,
        ObservationRoot::Window(&test_support::window()),
        &request,
    )
}

fn bounds() -> crate::Rect {
    crate::Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 30.0,
    }
}

fn surveyed_row(name: &str, description: &str, identifier: &str) -> LocatorEvidence {
    let mut row = evidence("button", Some(name));
    row.description = LocatorField::Known(description.into());
    row.value = LocatorField::Unknown;
    row.identifiers = IdentifierEvidence::typed(
        [crate::ElementIdentifier {
            kind: crate::IdentifierKind::AxIdentifier,
            value: identifier.into(),
        }],
        Some(0),
        true,
    );
    row.ref_evidence.bounds = LocatorField::Known(bounds());
    row.ref_evidence.available_actions =
        LocatorField::Known(vec![crate::actions::capability::CLICK.into()]);
    row
}

#[test]
fn selected_hydration_rechecks_every_recorded_identity_field() {
    let alpha = surveyed_row("Alpha", "first row", "row");
    let adapter_for = |hydrated: LocatorEvidence| RecheckAdapter {
        survey: vec![alpha.clone(), surveyed_row("Beta", "second row", "row")],
        hydrated,
    };
    let code = |hydrated: LocatorEvidence| {
        resolve_selected(&adapter_for(hydrated))
            .err()
            .map(|error| error.code().to_string())
    };
    let mut unchanged = alpha.clone();
    unchanged.value = LocatorField::Absent;

    let resolution = resolve_selected(&adapter_for(unchanged.clone())).unwrap();
    assert_eq!(resolution.matches[0].data.name, "Alpha");
    assert_eq!(
        resolution.matches[0].entry.identity.basis,
        Some(crate::IdentityBasis::Name)
    );

    let mut renamed = unchanged.clone();
    renamed.name = LocatorField::Known("Beta".into());
    assert_eq!(code(renamed).as_deref(), Some("STALE_REF"));
    let mut unread_name = unchanged.clone();
    unread_name.name = LocatorField::Unknown;
    assert_eq!(code(unread_name).as_deref(), Some("TIMEOUT"));

    let mut reordered = unchanged.clone();
    reordered.description = LocatorField::Known("second row".into());
    assert_eq!(code(reordered).as_deref(), Some("STALE_REF"));
    let mut unread_description = unchanged.clone();
    unread_description.description = LocatorField::Unknown;
    assert_eq!(code(unread_description).as_deref(), Some("TIMEOUT"));

    let mut other_group = unchanged.clone();
    other_group.identifiers = surveyed_row("Alpha", "first row", "row-2").identifiers;
    assert_eq!(code(other_group).as_deref(), Some("STALE_REF"));
    let mut unread_identifiers = unchanged;
    unread_identifiers.identifiers = IdentifierEvidence::unknown();
    assert_eq!(code(unread_identifiers).as_deref(), Some("TIMEOUT"));
}

fn unproven_button_evidence() -> LocatorEvidence {
    let mut evidence = evidence("button", None);
    evidence.name = LocatorField::Unknown;
    evidence.description = LocatorField::Unknown;
    evidence.value = LocatorField::Unknown;
    evidence.identifiers = IdentifierEvidence::typed([], None, true);
    evidence.ref_evidence.bounds = LocatorField::Known(bounds());
    evidence
}

#[test]
fn hydration_that_discovers_unproven_identity_records_an_unproven_basis() {
    let mut saved = evidence("button", Some("Save"));
    saved.ref_evidence.bounds = LocatorField::Known(bounds());
    saved.ref_evidence.available_actions =
        LocatorField::Known(vec![crate::actions::capability::CLICK.into()]);
    let adapter = RecheckAdapter {
        survey: vec![unproven_button_evidence()],
        hydrated: saved,
    };

    let resolution =
        resolve_selected(&adapter).expect("role-only selected hydration should still succeed");

    assert_eq!(resolution.matches[0].data.name, "Save");
    assert_eq!(
        resolution.matches[0].entry.identity.basis,
        Some(crate::IdentityBasis::Ambiguous),
        "the survey proved nothing about the name it never read"
    );
    assert_eq!(
        resolution
            .refmap
            .as_ref()
            .unwrap()
            .get("@e1")
            .unwrap()
            .identity
            .basis,
        Some(crate::IdentityBasis::Ambiguous)
    );
}
