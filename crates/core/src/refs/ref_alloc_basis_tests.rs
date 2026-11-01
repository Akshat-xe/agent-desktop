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
            skeleton_depth: None,
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

    let root = window_with(vec![
        entry("button", Some("Save"), Some("shared"), None),
        entry("button", Some("Cancel"), Some("shared"), None),
    ]);
    let (tree, refmap) = run(root, &allocation(None));
    assert_eq!(basis_of(&tree, &refmap, &[0]), Some(IdentityBasis::Name));
    assert_eq!(basis_of(&tree, &refmap, &[1]), Some(IdentityBasis::Name));

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

    let mut first = entry("button", None, Some("shared"), None);
    first.identity.value = Some("On".into());
    let mut second = entry("button", None, Some("shared"), None);
    second.identity.value = Some("Off".into());
    let (tree, refmap) = run(window_with(vec![first, second]), &allocation(None));
    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Ambiguous)
    );

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
/// elements would let the surviving ref resolve onto the inert twin. Look-alikes can also sit
/// under separate parent rows, so uniqueness is counted over the whole snapshot.
#[test]
fn an_inert_look_alike_of_the_same_role_and_name_withdraws_the_name_proof() {
    let mut first_row = node("group", Some("First row"));
    first_row.children = vec![entry("button", Some("Save"), None, None)];
    let mut second_row = node("group", Some("Second row"));
    second_row.children = vec![entry("button", Some("Save"), None, None)];
    let (tree, refmap) = run(window_with(vec![first_row, second_row]), &allocation(None));
    assert_eq!(
        basis_of(&tree, &refmap, &[0, 0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1, 0]),
        Some(IdentityBasis::Ambiguous)
    );

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
    let mut cut_row = node("group", Some("Unread row"));
    cut_row.subtree_truncated = true;
    let truncated = window_with(vec![
        entry("button", Some("Save"), Some("save-button"), None),
        cut_row,
    ]);
    let (tree, refmap) = run(truncated, &allocation(None));
    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );

    let drill = window_with(vec![
        entry("button", Some("Save"), Some("save-button"), None),
        entry("button", None, None, None),
    ]);
    let (tree, refmap) = run(drill.clone(), &allocation(Some("@e3")));
    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::NativeId)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Positional)
    );
    let (tree, refmap) = run(drill, &allocation_with(Some("@e3"), false));
    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous)
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Positional)
    );

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
