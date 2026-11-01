use crate::refs::ref_alloc::allocate_refs;
use crate::refs::ref_alloc_config::RefAllocConfig;
use crate::refs::ref_alloc_options::RefAllocOptions;
use crate::refs::ref_alloc_scope::RefAllocScope;
use crate::refs::ref_alloc_source::RefAllocSource;
use crate::{AccessibilityNode, RefMap};

fn named(role: &str, name: &str) -> AccessibilityNode {
    crate::adapter::minimal_accessibility_node(role, name)
}

fn window(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut root = named("window", "Fixture");
    root.children = children;
    root
}

fn config() -> RefAllocConfig<'static> {
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
        observation_complete: true,
    }
}

fn flags(root: AccessibilityNode) -> Vec<bool> {
    let mut refmap = RefMap::new();
    allocate_refs(root, &mut refmap, &config()).unwrap();
    (1..=refmap.len())
        .filter_map(|index| refmap.get(&format!("@e{index}")))
        .map(|entry| entry.identity.unique_at_capture)
        .collect()
}

#[test]
fn a_complete_observation_marks_the_entry_it_found_once() {
    let root = window(vec![
        named("button", "Send"),
        named("button", "Send"),
        named("button", "Cancel"),
    ]);

    assert_eq!(flags(root), [false, false, true]);
}
