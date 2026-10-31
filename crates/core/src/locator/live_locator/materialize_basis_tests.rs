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

#[test]
fn an_unread_field_withdraws_only_the_proofs_that_rest_on_it() {
    use crate::LocatorField;
    let mut unread_value_and_description = evidence("button", Some("Other"));
    unread_value_and_description.value = LocatorField::Unknown;
    unread_value_and_description.description = LocatorField::Unknown;
    let button = with_identifier(evidence("button", Some("Save")), "save-button");
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1, 2], &[]),
            node(1, unread_value_and_description, vec![], &[0]),
            node(2, button.clone(), vec![], &[1]),
        ],
        true,
    );
    assert_eq!(
        bases,
        [Some(IdentityBasis::Name), Some(IdentityBasis::NativeId)]
    );

    let mut unread_identifiers = evidence("button", Some("Other"));
    unread_identifiers.identifiers = IdentifierEvidence::typed([], None, false);
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1, 2], &[]),
            node(1, unread_identifiers, vec![], &[0]),
            node(2, button, vec![], &[1]),
        ],
        true,
    );
    assert_eq!(
        bases,
        [
            Some(IdentityBasis::Ambiguous),
            Some(IdentityBasis::Ambiguous)
        ]
    );
}

#[test]
fn an_unread_description_blocks_only_a_description_basis() {
    use crate::LocatorField;
    let mut described = with_identifier(evidence("button", None), "shared");
    described.description = LocatorField::Known("Archive".into());
    let mut unread = with_identifier(evidence("button", None), "shared");
    unread.description = LocatorField::Unknown;
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1, 2], &[]),
            node(1, described, vec![], &[0]),
            node(2, unread, vec![], &[1]),
        ],
        true,
    );
    assert_eq!(
        bases,
        [
            Some(IdentityBasis::Ambiguous),
            Some(IdentityBasis::Ambiguous)
        ]
    );
}

#[test]
fn identical_text_in_different_roles_earns_the_name_basis_in_both() {
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1, 2], &[]),
            node(1, evidence("button", Some("Save")), vec![], &[0]),
            node(2, evidence("link", Some("Save")), vec![], &[1]),
        ],
        true,
    );

    assert_eq!(
        bases,
        [Some(IdentityBasis::Name), Some(IdentityBasis::Name)],
        "identical text in another role is not a look-alike"
    );
}

#[test]
fn identical_text_in_different_identifier_groups_is_not_ambiguous() {
    let save = with_identifier(evidence("button", Some("Save")), "shared");
    let cancel = with_identifier(evidence("button", Some("Cancel")), "shared");
    let elsewhere = with_identifier(evidence("button", Some("Save")), "elsewhere");
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1, 2, 3], &[]),
            node(1, save, vec![], &[0]),
            node(2, cancel, vec![], &[1]),
            node(3, elsewhere, vec![], &[1]),
        ],
        true,
    );

    assert_eq!(
        bases,
        [
            Some(IdentityBasis::Name),
            Some(IdentityBasis::Name),
            Some(IdentityBasis::NativeId)
        ],
        "identical text under another identifier is not a look-alike"
    );
}

#[test]
fn an_unread_name_on_another_element_leaves_a_unique_identifier_proven() {
    let mut unreadable = evidence("button", Some("Cancel"));
    unreadable.name = crate::LocatorField::Unknown;
    let bases = materialized_bases(
        vec![
            node(0, evidence("window", Some("Fixture")), vec![1, 2], &[]),
            node(
                1,
                with_identifier(evidence("button", Some("Save")), "save-button"),
                vec![],
                &[0],
            ),
            node(2, unreadable, vec![], &[1]),
        ],
        true,
    );

    assert_eq!(
        bases[0],
        Some(IdentityBasis::NativeId),
        "a look-alike whose identifiers were read cannot share this identifier"
    );
}
