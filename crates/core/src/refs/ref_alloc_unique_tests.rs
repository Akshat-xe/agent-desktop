use super::*;
use crate::refs::ref_alloc::allocate_refs;
use crate::refs::ref_alloc_config::RefAllocConfig;
use crate::refs::ref_alloc_options::RefAllocOptions;
use crate::refs::ref_alloc_scope::RefAllocScope;
use crate::refs::ref_alloc_source::RefAllocSource;
use crate::{AccessibilityNode, RefMap};

fn element(role: &str) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: Default::default(),
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    }
}

fn named(role: &str, name: &str) -> AccessibilityNode {
    let mut node = element(role);
    node.identity.name = Some(name.into());
    node
}

fn valued(role: &str, name: &str, value: &str) -> AccessibilityNode {
    let mut node = named(role, name);
    node.identity.value = Some(value.into());
    node
}

fn window(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut root = named("window", "Fixture");
    root.children = children;
    root
}

fn config(observation_complete: bool) -> RefAllocConfig<'static> {
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
        observation_complete,
    }
}

fn flags(root: AccessibilityNode, config: &RefAllocConfig<'_>) -> Vec<bool> {
    let mut refmap = RefMap::new();
    allocate_refs(root, &mut refmap, config).unwrap();
    (1..=refmap.len())
        .filter_map(|index| refmap.get(&format!("@e{index}")))
        .map(|entry| entry.identity.unique_at_capture)
        .collect()
}

#[test]
fn one_named_button_is_unique() {
    let root = window(vec![named("button", "Send")]);

    assert_eq!(unique_capture_paths(&root), HashSet::from([vec![0]]));
}

#[test]
fn identical_twins_are_not_unique() {
    let root = window(vec![named("button", "Send"), named("button", "Send")]);

    assert!(unique_capture_paths(&root).is_empty());
}

#[test]
fn a_twin_that_cannot_take_a_ref_still_counts() {
    let mut actionable = named("group", "Send");
    actionable.presentation.available_actions = vec![crate::actions::capability::CLICK.into()];
    let root = window(vec![actionable, named("group", "Send")]);

    assert!(unique_capture_paths(&root).is_empty());
}

#[test]
fn a_name_mirroring_a_mutable_value_proves_no_uniqueness() {
    let root = window(vec![
        valued("slider", "50", "50"),
        valued("slider", "60", "60"),
    ]);

    assert!(unique_capture_paths(&root).is_empty());
}

#[test]
fn a_name_that_differs_from_its_value_is_stable_identity() {
    let root = window(vec![
        valued("slider", "Volume", "10"),
        valued("slider", "Balance", "20"),
    ]);

    assert_eq!(
        unique_capture_paths(&root),
        HashSet::from([vec![0], vec![1]])
    );
}

#[test]
fn omitted_descendants_prove_no_uniqueness() {
    let mut truncated = named("button", "Send");
    truncated.subtree_truncated = true;
    let mut skeleton = named("group", "Sidebar");
    skeleton.children_count = Some(4);

    assert!(unique_capture_paths(&window(vec![truncated])).is_empty());
    assert!(unique_capture_paths(&window(vec![skeleton])).is_empty());
}

#[test]
fn a_complete_observation_marks_the_entry_it_found_once() {
    let root = window(vec![
        named("button", "Send"),
        named("button", "Send"),
        named("button", "Cancel"),
    ]);

    assert_eq!(flags(root, &config(true)), [false, false, true]);
}

#[test]
fn an_incomplete_observation_marks_nothing() {
    let root = window(vec![named("button", "Send")]);

    assert_eq!(flags(root, &config(false)), [false]);
}

#[test]
fn a_drill_down_marks_nothing() {
    let root = window(vec![named("button", "Send")]);
    let mut drill = config(true);
    drill.scope.root_ref_id = Some("@e3");

    assert_eq!(flags(root, &drill), [false]);
}
