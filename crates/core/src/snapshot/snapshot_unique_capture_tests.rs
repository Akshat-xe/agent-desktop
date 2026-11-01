use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps, TreeOptions, WindowFilter};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::{AccessibilityNode, AdapterError, WindowInfo};

struct FixtureAdapter {
    tree: AccessibilityNode,
}

impl ObservationOps for FixtureAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        crate::adapter::observed_tree(&root, self.tree.clone())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "FixtureApp".into(),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }
}

impl ActionOps for FixtureAdapter {}

impl InputOps for FixtureAdapter {}

impl SystemOps for FixtureAdapter {
    fn supported_surfaces(&self) -> Vec<crate::SnapshotSurface> {
        vec![crate::SnapshotSurface::Window]
    }
}

fn element(role: &str, name: Option<&str>) -> AccessibilityNode {
    let mut node = AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: Default::default(),
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    };
    node.identity.name = name.map(str::to_string);
    node
}

fn snapshot(tree: AccessibilityNode) -> (bool, Vec<bool>) {
    let result = crate::snapshot::build(
        &FixtureAdapter { tree },
        &TreeOptions::default(),
        None,
        None,
        crate::Deadline::after(1_000).unwrap(),
    )
    .unwrap();
    let flags = (1..=result.refmap.len())
        .filter_map(|index| result.refmap.get(&format!("@e{index}")))
        .map(|entry| entry.identity.unique_at_capture)
        .collect();
    (result.complete, flags)
}

#[test]
fn a_complete_snapshot_marks_the_one_entry_it_found_once() {
    let mut root = element("window", Some("Fixture"));
    root.children = vec![
        element("button", Some("Send")),
        element("button", Some("Send")),
        element("button", Some("Cancel")),
    ];

    assert_eq!(snapshot(root), (true, vec![false, false, true]));
}

#[test]
fn an_incomplete_snapshot_marks_nothing() {
    let mut truncated = element("button", Some("Cancel"));
    truncated.subtree_truncated = true;
    let mut root = element("window", Some("Fixture"));
    root.children = vec![element("button", Some("Send")), truncated];

    assert_eq!(snapshot(root), (false, vec![false, false]));
}
