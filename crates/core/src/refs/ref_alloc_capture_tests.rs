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
            census: None,
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

/// A count keyed by identifier group would see the identifier-free textfield alone, so a
/// live name match would let the identified twin replace its ref.
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

/// A live native-id match accepts any identifier a button exposes, so the one exposing
/// `shared` beside its preferred identifier is a look-alike even though the projection drops it.
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

/// A subtree cut at the requested depth never entered the census, so a proof counting only
/// what was walked lets the hidden twin replace the visible target.
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

    let mut unreadable = evidence("button", None);
    unreadable.name = LocatorField::Unknown;
    let (tree, refmap) = capture(snapshot(vec![node(
        "window",
        "Fixture",
        vec![leaf(evidence("button", Some("Save"))), leaf(unreadable)],
    )]));

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

#[test]
fn value_mirroring_names_still_count_as_capture_time_look_alikes() {
    for identifiers in [&[][..], &[("ax", "shared")][..]] {
        for value in ["10", "10.0"] {
            let mut target = identified("textfield", Some("10"), identifiers);
            target.value = known(Some("20"));
            let mut competitor = identified("textfield", Some("10"), identifiers);
            competitor.value = known(Some(value));
            let (tree, refmap) = capture(snapshot(vec![node(
                "window",
                "Fixture",
                vec![leaf(target), leaf(competitor)],
            )]));
            let saved = refmap
                .get(tree.children[0].ref_id.as_deref().unwrap())
                .unwrap();

            assert_eq!(saved.identity.basis, Some(IdentityBasis::Ambiguous));
            assert_eq!(
                basis_of(&tree, &refmap, &[1]),
                Some(if identifiers.is_empty() {
                    IdentityBasis::Positional
                } else {
                    IdentityBasis::Ambiguous
                })
            );
            let error = crate::refs::identity_basis_gate::basis_gate(&saved.identity).unwrap_err();
            assert_eq!(error.code, crate::ErrorCode::AmbiguousTarget);
            assert_eq!(error.details.unwrap()["kind"], "identity_not_unique");
            assert!(!crate::refs::ref_identity::identity_matches(
                saved,
                Some("10"),
                Some("30"),
                None,
                identifiers.first().map(|(_, value)| *value),
            ));
        }
    }

    for (value, expected) in [
        ("10", IdentityBasis::Positional),
        ("10.0", IdentityBasis::Positional),
        ("20", IdentityBasis::Name),
    ] {
        let mut target = evidence("textfield", Some("10"));
        target.value = known(Some(value));
        let (tree, refmap) = capture(snapshot(vec![node(
            "window",
            "Fixture",
            vec![leaf(target)],
        )]));

        assert_eq!(basis_of(&tree, &refmap, &[0]), Some(expected));
    }
}

#[test]
fn a_mutable_name_whose_value_was_unread_is_not_saved() {
    let mut field = identified("textfield", Some("draft"), &[("ax", "field")]);
    field.value = LocatorField::Unknown;
    let (tree, refmap) = capture(snapshot(vec![node("window", "Fixture", vec![leaf(field)])]));
    let saved = refmap
        .get(tree.children[0].ref_id.as_deref().unwrap())
        .unwrap();

    assert_eq!(saved.identity.basis, Some(IdentityBasis::NativeId));
    assert_eq!(saved.identity.name, None);
}
