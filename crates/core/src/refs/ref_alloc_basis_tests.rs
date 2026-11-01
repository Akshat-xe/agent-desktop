use super::*;
use crate::IdentityBasis;

fn entry(
    role: &str,
    name: Option<&str>,
    identifier: Option<&str>,
    description: Option<&str>,
) -> AccessibilityNode {
    let mut built = node(role, name);
    built.identity.native_id = identifier.map(|value| crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: value.into(),
    });
    built.identity.description = description.map(str::to_string);
    built
}

fn allocation(root_ref_id: Option<&str>) -> RefAllocConfig<'_> {
    allocation_with(root_ref_id, true)
}

fn allocation_with(root_ref_id: Option<&str>, complete: bool) -> RefAllocConfig<'_> {
    RefAllocConfig {
        options: crate::refs::ref_alloc_options::RefAllocOptions {
            include_bounds: true,
            interactive_only: false,
            compact: false,
        },
        source: crate::refs::ref_alloc_source::RefAllocSource {
            pid: crate::ProcessId::new(7),
            app: None,
            window_id: None,
            window_title: None,
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::refs::ref_alloc_scope::RefAllocScope {
            root_ref_id,
            path_prefix: &[],
        },
        complete,
    }
}

fn run(root: AccessibilityNode, config: &RefAllocConfig<'_>) -> (AccessibilityNode, RefMap) {
    let mut refmap = RefMap::new();
    let tree = allocate_refs(root, &mut refmap, config).unwrap();
    (tree, refmap)
}

fn basis_of(tree: &AccessibilityNode, refmap: &RefMap, path: &[usize]) -> Option<IdentityBasis> {
    let node = path
        .iter()
        .try_fold(tree, |current, index| current.children.get(*index))?;
    refmap
        .get(node.ref_id.as_deref()?)
        .and_then(|entry| entry.identity.basis)
}

fn window_with(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut root = node("window", Some("w"));
    root.children = children;
    root
}

#[test]
fn a_unique_identifier_is_the_native_id_basis() {
    let root = window_with(vec![entry(
        "button",
        Some("Save"),
        Some("save-button"),
        None,
    )]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::NativeId)
    );
}

#[test]
fn a_shared_identifier_with_a_unique_name_is_the_name_basis() {
    let root = window_with(vec![
        entry("button", Some("Save"), Some("shared"), None),
        entry("button", Some("Cancel"), Some("shared"), None),
    ]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(basis_of(&tree, &refmap, &[0]), Some(IdentityBasis::Name));
    assert_eq!(basis_of(&tree, &refmap, &[1]), Some(IdentityBasis::Name));
}

#[test]
fn a_shared_identifier_with_a_unique_description_is_the_description_basis() {
    let root = window_with(vec![
        entry("button", None, Some("shared"), Some("Saves the file")),
        entry("button", None, Some("shared"), Some("Discards changes")),
    ]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Description)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Description)
    );
}

/// A control value changes as the user interacts, so two entries that differ
/// only in value are look-alikes, not a name and a native identifier.
#[test]
fn a_shared_identifier_differing_only_in_value_is_ambiguous() {
    let mut first = entry("button", None, Some("shared"), None);
    first.identity.value = Some("On".into());
    let mut second = entry("button", None, Some("shared"), None);
    second.identity.value = Some("Off".into());
    let root = window_with(vec![first, second]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Ambiguous)
    );
}

#[test]
fn an_entry_with_no_identifier_and_no_text_is_positional() {
    let root = window_with(vec![
        entry("button", None, None, None),
        entry("button", None, None, None),
    ]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Positional)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Positional)
    );
}

#[test]
fn id_less_entries_are_ambiguous_by_shared_name_and_named_by_distinct_ones() {
    let shared = window_with(vec![
        entry("button", Some("Save"), None, None),
        entry("button", Some("Save"), None, None),
    ]);
    let (tree, refmap) = run(shared, &allocation(None));
    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Ambiguous)
    );

    let distinct = window_with(vec![
        entry("button", Some("Save"), None, None),
        entry("button", Some("Cancel"), None, None),
    ]);
    let (tree, refmap) = run(distinct, &allocation(None));
    assert_eq!(basis_of(&tree, &refmap, &[0]), Some(IdentityBasis::Name));
    assert_eq!(basis_of(&tree, &refmap, &[1]), Some(IdentityBasis::Name));
}

/// Look-alikes can sit under separate parent rows, so uniqueness must be
/// counted over the whole snapshot rather than among siblings.
#[test]
fn cousins_under_separate_rows_count_as_look_alikes() {
    let mut first_row = node("group", Some("First row"));
    first_row.children = vec![entry("button", Some("Save"), None, None)];
    let mut second_row = node("group", Some("Second row"));
    second_row.children = vec![entry("button", Some("Save"), None, None)];
    let root = window_with(vec![first_row, second_row]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0, 0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1, 0]),
        Some(IdentityBasis::Ambiguous)
    );
}

#[test]
fn a_truncated_tree_is_ambiguous_even_when_the_observed_subset_looks_unique() {
    let mut cut_row = node("group", Some("Unread row"));
    cut_row.subtree_truncated = true;
    let root = window_with(vec![
        entry("button", Some("Save"), Some("save-button"), None),
        cut_row,
    ]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
}

#[test]
fn a_complete_drill_proves_identity_but_an_incomplete_drill_does_not() {
    let root = window_with(vec![
        entry("button", Some("Save"), Some("save-button"), None),
        entry("button", None, None, None),
    ]);

    let (tree, refmap) = run(root.clone(), &allocation(Some("@e3")));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::NativeId)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Positional)
    );
    let (tree, refmap) = run(root, &allocation_with(Some("@e3"), false));
    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Positional)
    );
}

#[test]
fn an_incomplete_snapshot_is_ambiguous_even_when_the_tree_looks_whole() {
    let root = window_with(vec![entry(
        "button",
        Some("Save"),
        Some("save-button"),
        None,
    )]);

    let (tree, refmap) = run(root, &allocation_with(None, false));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
}

#[test]
fn identical_text_across_roles_never_becomes_ambiguous() {
    let root = window_with(vec![
        entry("button", Some("Save"), None, None),
        entry("cell", Some("Save"), None, None),
        entry("button", Some("Save"), None, None),
    ]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(basis_of(&tree, &refmap, &[1]), Some(IdentityBasis::Name));
    assert_eq!(
        basis_of(&tree, &refmap, &[2]),
        Some(IdentityBasis::Ambiguous)
    );
}

#[test]
fn identical_text_across_identifier_groups_never_becomes_ambiguous() {
    let root = window_with(vec![
        entry("button", Some("Save"), Some("row-a"), None),
        entry("button", Some("Save"), Some("row-a"), None),
        entry("button", Some("Save"), Some("row-b"), None),
        entry("button", Some("Cancel"), Some("row-b"), None),
    ]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(basis_of(&tree, &refmap, &[2]), Some(IdentityBasis::Name));
    assert_eq!(basis_of(&tree, &refmap, &[3]), Some(IdentityBasis::Name));
}

/// An inert group named Row is a look-alike of the actionable one, so counting only ref-able
/// elements would let the surviving ref resolve onto the inert twin.
#[test]
fn an_inert_look_alike_of_the_same_role_and_name_withdraws_the_name_proof() {
    let mut actionable = node("group", Some("Row"));
    actionable.presentation.available_actions = vec![crate::actions::capability::CLICK.to_string()];
    let inert = node("group", Some("Row"));
    let root = window_with(vec![actionable, inert]);

    let (tree, refmap) = run(root, &allocation(None));

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous),
        "the inert twin shares the actionable group's role and name"
    );
    assert!(
        tree.children[1].ref_id.is_none(),
        "eligibility still controls which elements receive a ref"
    );
    let captured = refmap
        .get(tree.children[0].ref_id.as_deref().unwrap())
        .unwrap();
    let error = crate::refs::identity_basis_gate::basis_gate(&captured.identity).unwrap_err();
    assert_eq!(error.code, crate::ErrorCode::AmbiguousTarget);
    assert_eq!(error.details.unwrap()["kind"], "identity_not_unique");
}

#[test]
fn an_incomplete_capture_is_refused_with_ambiguous_target_before_any_live_search() {
    let root = window_with(vec![entry(
        "button",
        Some("Save"),
        Some("save-button"),
        None,
    )]);

    let (tree, refmap) = run(root, &allocation_with(None, false));
    let captured = refmap
        .get(tree.children[0].ref_id.as_deref().unwrap())
        .unwrap();

    assert_eq!(captured.identity.basis, Some(IdentityBasis::Ambiguous));
    assert!(!crate::refs::identity_basis_gate::requires_live_uniqueness(
        &captured.identity
    ));
    let error = crate::refs::identity_basis_gate::basis_gate(&captured.identity).unwrap_err();
    assert_eq!(error.code, crate::ErrorCode::AmbiguousTarget);
    assert_eq!(error.details.unwrap()["kind"], "identity_not_unique");
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
}
