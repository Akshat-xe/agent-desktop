use super::test_support::{evidence, node, tree};
use super::{
    IdentifierEvidence, LocatorEvidence, LocatorMaterialization, LocatorResolveRequest,
    LocatorSelection, ObservedNode, evaluate_locator_tree,
};
use crate::IdentityBasis;
use crate::locator::LocatorQuery;

fn request() -> LocatorResolveRequest {
    LocatorResolveRequest {
        selection: LocatorSelection::Strict,
        deadline: crate::Deadline::from_duration(std::time::Duration::from_secs(5)).unwrap(),
        max_raw_depth: 50,
        surface: None,
        materialization: LocatorMaterialization::FullRefMap,
    }
}

fn with_identifier(mut evidence: LocatorEvidence, value: &str) -> LocatorEvidence {
    evidence.identifiers = IdentifierEvidence::typed(
        [crate::ElementIdentifier {
            kind: crate::IdentifierKind::AxIdentifier,
            value: value.into(),
        }],
        Some(0),
        true,
    );
    evidence
}

fn materialized_bases(nodes: Vec<ObservedNode>, complete: bool) -> Vec<Option<IdentityBasis>> {
    let resolution = evaluate_locator_tree(
        tree(nodes, vec![0], complete),
        &LocatorQuery::default(),
        &request(),
    )
    .unwrap();
    let refmap = resolution.refmap.unwrap();
    (1..=refmap.len())
        .map(|index| {
            refmap
                .get(&format!("@e{index}"))
                .and_then(|entry| entry.identity.basis)
        })
        .collect()
}

#[test]
fn a_unique_identifier_is_recorded_as_the_native_id_basis() {
    let button = with_identifier(evidence("button", Some("Save")), "save-button");
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1], &[]),
            node(1, button, vec![], &[0]),
        ],
        true,
    );

    assert_eq!(bases, [Some(IdentityBasis::NativeId)]);
}

#[test]
fn a_shared_identifier_with_distinct_names_is_recorded_as_the_name_basis() {
    let first = with_identifier(evidence("button", Some("Save")), "shared");
    let second = with_identifier(evidence("button", Some("Cancel")), "shared");
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1, 2], &[]),
            node(1, first, vec![], &[0]),
            node(2, second, vec![], &[1]),
        ],
        true,
    );

    assert_eq!(
        bases,
        [Some(IdentityBasis::Name), Some(IdentityBasis::Name)]
    );
}

#[test]
fn an_entry_without_identifier_or_text_is_recorded_as_positional() {
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1], &[]),
            node(1, evidence("button", None), vec![], &[0]),
        ],
        true,
    );

    assert_eq!(bases, [Some(IdentityBasis::Positional)]);
}

#[test]
fn an_incomplete_observation_records_ambiguous_even_for_a_unique_identifier() {
    let button = with_identifier(evidence("button", Some("Save")), "save-button");
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1], &[]),
            node(1, button, vec![], &[0]),
        ],
        false,
    );

    assert_eq!(bases, [Some(IdentityBasis::Ambiguous)]);
}
