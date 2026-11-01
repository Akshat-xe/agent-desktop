use std::collections::HashSet;

use crate::locator::live_locator::{
    IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, LocatorStats,
    ObservationRoot, ObservationSource, ObservedSubtree, ObservedTree,
};
use crate::refs::ref_alloc::allocate_refs;
use crate::refs::ref_alloc_config::RefAllocConfig;
use crate::refs::ref_alloc_options::RefAllocOptions;
use crate::refs::ref_alloc_scope::RefAllocScope;
use crate::refs::ref_alloc_source::RefAllocSource;
use crate::refs::ref_alloc_unique::unique_capture_paths;
use crate::{AccessibilityNode, ElementIdentifier, IdentifierKind, RefMap, WindowInfo};

fn named(role: &str, name: &str) -> AccessibilityNode {
    crate::adapter::minimal_accessibility_node(role, name)
}

fn window(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut root = named("window", "Fixture");
    root.children = children;
    root
}

fn fixture_window() -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Fixture".into(),
        app: "Fixture".into(),
        pid: crate::ProcessId::new(1),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState::default(),
    }
}

fn config(unique: &HashSet<Vec<usize>>) -> RefAllocConfig<'_> {
    RefAllocConfig {
        options: RefAllocOptions {
            include_bounds: false,
            interactive_only: false,
            compact: false,
        },
        source: RefAllocSource {
            pid: crate::ProcessId::new(1),
            app: Some("Test"),
            window_id: None,
            window_title: Some("Test Window"),
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::SnapshotSurface::Window,
        },
        scope: RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
        unique_entries: Some(unique),
    }
}

fn flags(root: AccessibilityNode, observed: &ObservedTree) -> Vec<bool> {
    let unique = unique_capture_paths(observed);
    let mut refmap = RefMap::new();
    allocate_refs(root, &mut refmap, &config(&unique)).unwrap();
    (1..=refmap.len())
        .filter_map(|index| refmap.get(&format!("@e{index}")))
        .map(|entry| entry.identity.unique_at_capture)
        .collect()
}

fn observed_from(root: AccessibilityNode) -> ObservedTree {
    let window = fixture_window();
    crate::adapter::observed_tree(&ObservationRoot::Window(&window), root)
        .expect("the fixture observes")
}

fn evidence(
    role: &str,
    name: &str,
    ids: Vec<ElementIdentifier>,
    preferred: usize,
) -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known(role.into()),
        name: LocatorField::Known(name.into()),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: IdentifierEvidence::typed(ids, Some(preferred), true),
        states: LocatorField::Known(Vec::new()),
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Known(Vec::new()),
            descriptors: Default::default(),
        },
    }
}

fn identifier(kind: IdentifierKind, value: &str) -> ElementIdentifier {
    ElementIdentifier {
        kind,
        value: value.into(),
    }
}

fn twin_carrying_the_identifier_secondhand() -> (AccessibilityNode, ObservedTree) {
    let mut captured = named("button", "Send");
    captured.identity.native_id = Some(identifier(IdentifierKind::AxIdentifier, "x-1"));
    let mut twin = named("button", "Send");
    twin.identity.native_id = Some(identifier(IdentifierKind::AxDomIdentifier, "dom-2"));

    let observed = ObservedTree::from_roots(
        vec![ObservedSubtree::new(
            evidence("window", "Fixture", Vec::new(), 0),
            vec![
                ObservedSubtree::new(
                    evidence(
                        "button",
                        "Send",
                        vec![identifier(IdentifierKind::AxIdentifier, "x-1")],
                        0,
                    ),
                    Vec::new(),
                    true,
                    None,
                ),
                ObservedSubtree::new(
                    evidence(
                        "button",
                        "Send",
                        vec![
                            identifier(IdentifierKind::AxDomIdentifier, "dom-2"),
                            identifier(IdentifierKind::AxIdentifier, "x-1"),
                        ],
                        0,
                    ),
                    Vec::new(),
                    true,
                    None,
                ),
            ],
            true,
            None,
        )],
        ObservationSource::Window {
            window: fixture_window(),
            surface: crate::SnapshotSurface::Window,
        },
        LocatorStats::default(),
        true,
    )
    .expect("the fixture observes");

    (window(vec![captured, twin]), observed)
}

#[test]
fn a_complete_observation_marks_the_entry_it_found_once() {
    let root = window(vec![
        named("button", "Send"),
        named("button", "Send"),
        named("button", "Cancel"),
    ]);
    let observed = observed_from(root.clone());

    assert_eq!(flags(root, &observed), [false, false, true]);
}

#[test]
fn an_identifier_a_twin_carries_secondhand_is_not_unique() {
    let (root, observed) = twin_carrying_the_identifier_secondhand();

    assert_eq!(flags(root, &observed), [false, true]);
}

#[test]
fn find_marks_only_a_whole_window_match_found_once() {
    let observed = observed_from(window(vec![
        named("button", "Send"),
        named("button", "Send"),
        named("button", "Cancel"),
    ]));
    let unique = super::unique_capture_nodes(&observed);
    let named_unique: Vec<_> = unique
        .iter()
        .map(|index| &observed.nodes[*index].evidence)
        .filter(|evidence| evidence.role.known().map(String::as_str) == Some("button"))
        .filter_map(|evidence| evidence.name.meaningful_string())
        .collect();
    assert_eq!(named_unique, ["Cancel"]);
}
