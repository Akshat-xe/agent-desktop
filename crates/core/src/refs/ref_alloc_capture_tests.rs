use super::*;
use crate::locator::live_locator::{
    IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, LocatorStats,
    ObservationSource, ObservedSubtree, ObservedTree,
};
use crate::{ElementIdentifier, IdentityBasis};

fn known(value: Option<&str>) -> LocatorField<String> {
    value
        .map(|value| LocatorField::Known(value.to_string()))
        .unwrap_or(LocatorField::Absent)
}

fn evidence(role: &str, name: Option<&str>) -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known(role.into()),
        name: known(name),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Known(Vec::new()),
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Known(Vec::new()),
            descriptors: Default::default(),
        },
    }
}

fn identified(role: &str, name: Option<&str>, identifiers: &[(&str, &str)]) -> LocatorEvidence {
    let mut evidence = evidence(role, name);
    let typed: Vec<ElementIdentifier> = identifiers
        .iter()
        .map(|(kind, value)| ElementIdentifier {
            kind: match *kind {
                "ax_dom" => crate::IdentifierKind::AxDomIdentifier,
                _ => crate::IdentifierKind::AxIdentifier,
            },
            value: (*value).into(),
        })
        .collect();
    let preferred = identifiers.len().checked_sub(1);
    evidence.identifiers = IdentifierEvidence::typed(typed, preferred, true);
    evidence
}

fn leaf(evidence: LocatorEvidence) -> ObservedSubtree {
    ObservedSubtree::new(evidence, Vec::new(), true, None)
}

fn node(role: &str, name: &str, children: Vec<ObservedSubtree>) -> ObservedSubtree {
    ObservedSubtree::new(evidence(role, Some(name)), children, true, None)
}

fn snapshot(roots: Vec<ObservedSubtree>) -> ObservedTree {
    let window = crate::WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "FixtureApp".into(),
        pid: crate::ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            ..Default::default()
        },
    };
    ObservedTree::from_roots(
        roots,
        ObservationSource::Window {
            window,
            surface: crate::adapter::SnapshotSurface::Window,
        },
        LocatorStats::default(),
        true,
    )
    .unwrap()
}

fn allocation() -> RefAllocConfig<'static> {
    RefAllocConfig {
        options: crate::refs::ref_alloc_options::RefAllocOptions {
            include_bounds: true,
            interactive_only: false,
            compact: false,
        },
        source: crate::refs::ref_alloc_source::RefAllocSource {
            pid: crate::ProcessId::new(42),
            app: None,
            window_id: None,
            window_title: None,
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::adapter::SnapshotSurface::Window,
        },
        scope: crate::refs::ref_alloc_scope::RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
        complete: true,
    }
}

fn basis_of(tree: &AccessibilityNode, refmap: &RefMap, path: &[usize]) -> Option<IdentityBasis> {
    let node = path
        .iter()
        .try_fold(tree, |current, index| current.children.get(*index))?;
    refmap
        .get(node.ref_id.as_deref()?)
        .and_then(|entry| entry.identity.basis)
}

fn capture(observed: ObservedTree) -> (AccessibilityNode, RefMap) {
    let (root, complete, _) = observed.into_accessibility_tree_partial().unwrap();
    assert!(complete);
    let mut refmap = RefMap::new();
    let tree = allocate_refs(root, &mut refmap, &allocation()).unwrap();
    (tree, refmap)
}

/// Textfield A carries no identifier and textfield B carries one, so a count
/// keyed by identifier group sees A alone. A live name match ignores
/// identifiers it was not told about, so B replaces A's ref once A is gone.
#[test]
fn an_identifier_free_text_identity_counts_every_element_of_its_role() {
    let observed = snapshot(vec![node(
        "window",
        "Fixture",
        vec![
            leaf(evidence("textfield", Some("Address"))),
            leaf(identified(
                "textfield",
                Some("Address"),
                &[("ax", "field-b")],
            )),
        ],
    )]);

    let (tree, refmap) = capture(observed);

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous),
        "the identifier-free name also matches the identified look-alike"
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::NativeId),
        "the identified look-alike keeps its own unique identifier"
    );
}

/// Save button A exposes `shared`, and B exposes it beside a preferred DOM
/// identifier. A live native-id match accepts either of B's identifiers, so
/// B is a look-alike of A even though the projection keeps only B's preferred
/// one.
#[test]
fn an_identifier_exposed_beside_the_preferred_one_counts_as_a_look_alike() {
    let observed = snapshot(vec![node(
        "window",
        "Fixture",
        vec![
            leaf(identified("button", Some("Save"), &[("ax", "shared")])),
            leaf(identified(
                "button",
                Some("Save"),
                &[("ax", "shared"), ("ax_dom", "save-b")],
            )),
        ],
    )]);

    let (tree, refmap) = capture(observed);

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous),
        "A's recorded identifier also matches B"
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::NativeId),
        "B's preferred identifier is its own"
    );
}

/// The observation stopped at the requested depth and reported itself
/// complete, so the duplicate Save under the unread row never entered the
/// census. A proof that counts only what was walked lets the hidden twin
/// replace the visible target.
#[test]
fn an_omitted_subtree_withdraws_the_identity_proof_of_a_visible_look_alike() {
    let visible = || {
        node(
            "group",
            "First row",
            vec![leaf(evidence("button", Some("Save")))],
        )
    };
    let omitted = || {
        ObservedSubtree::new(
            evidence("group", Some("Second row")),
            Vec::new(),
            true,
            Some(1),
        )
    };

    let (tree, refmap) = capture(snapshot(vec![node(
        "window",
        "Fixture",
        vec![visible(), omitted()],
    )]));

    assert_eq!(
        basis_of(&tree, &refmap, &[0, 0]),
        Some(IdentityBasis::Ambiguous),
        "an omitted subtree could hide a duplicate"
    );

    let (tree, refmap) = capture(snapshot(vec![node(
        "window",
        "Fixture",
        vec![visible(), leaf(evidence("group", Some("Second row")))],
    )]));

    assert_eq!(
        basis_of(&tree, &refmap, &[0, 0]),
        Some(IdentityBasis::Name),
        "a fully walked tree keeps the proof"
    );
}

/// The second Save's name read failed, which the projection erases to an
/// absent name. Counting the first as unique lets it resolve onto the second
/// once that one is readable.
#[test]
fn an_unreadable_name_on_a_look_alike_withdraws_the_other_name_proof() {
    let mut unreadable = evidence("button", None);
    unreadable.name = LocatorField::Unknown;
    let observed = snapshot(vec![node(
        "window",
        "Fixture",
        vec![leaf(evidence("button", Some("Save"))), leaf(unreadable)],
    )]);

    let (tree, refmap) = capture(observed);

    assert_eq!(
        basis_of(&tree, &refmap, &[0]),
        Some(IdentityBasis::Ambiguous),
        "the unreadable name could be a duplicate of this one"
    );
    assert_eq!(
        basis_of(&tree, &refmap, &[1]),
        Some(IdentityBasis::Positional),
        "an element with no readable identity is positional"
    );
}
