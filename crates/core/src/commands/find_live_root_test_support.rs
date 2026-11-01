use crate::{
    AdapterError, WindowInfo,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter},
    locator::live_locator::{
        IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, LocatorStats,
        ObservationRequest, ObservationRoot, ObservationSource, ObservedSubtree, ObservedTree,
    },
    refs::RefEntry,
};

thread_local! {
    static MOVED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Makes the fixture shift "Panel" from index 0 to 1 behind an "Other" group.
pub(crate) fn move_panel() {
    MOVED.with(|moved| moved.set(true));
}

struct Spec {
    role: &'static str,
    name: Option<&'static str>,
    children: Vec<Spec>,
}

fn spec(role: &'static str, name: Option<&'static str>, children: Vec<Spec>) -> Spec {
    Spec {
        role,
        name,
        children,
    }
}

/// A window holding a "Panel" group. The panel owns a unique "Save" button, an unnamed field and
/// two "Dup" buttons. A second "Save" button sits outside the panel.
fn ui() -> Spec {
    if MOVED.with(std::cell::Cell::get) {
        return spec(
            "window",
            Some("Fixture"),
            vec![
                spec(
                    "group",
                    Some("Other"),
                    vec![spec("textfield", Some("Address"), vec![])],
                ),
                spec(
                    "group",
                    Some("Panel"),
                    vec![spec("textfield", Some("Address"), vec![])],
                ),
            ],
        );
    }
    spec(
        "window",
        Some("Fixture"),
        vec![
            spec(
                "group",
                Some("Panel"),
                vec![
                    spec("button", Some("Save"), vec![]),
                    spec("textfield", None, vec![]),
                    spec("button", Some("Dup"), vec![]),
                    spec("button", Some("Dup"), vec![]),
                ],
            ),
            spec("button", Some("Save"), vec![]),
        ],
    )
}

fn at<'a>(root: &'a Spec, path: &[usize]) -> Option<&'a Spec> {
    path.iter()
        .try_fold(root, |node, index| node.children.get(*index))
}

fn bounds(path: &[usize]) -> crate::Rect {
    let x = path
        .iter()
        .fold(0_u32, |acc, index| acc * 7 + *index as u32 + 1);
    crate::Rect {
        x: f64::from(x),
        y: 1.0,
        width: 10.0,
        height: 10.0,
    }
}

fn evidence(node: &Spec, path: &[usize]) -> LocatorEvidence {
    let actions = if node.role == "group" {
        vec!["press".to_string()]
    } else {
        Vec::new()
    };
    LocatorEvidence {
        role: LocatorField::Known(node.role.into()),
        name: node
            .name
            .map(|name| LocatorField::Known(name.into()))
            .unwrap_or(LocatorField::Absent),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Known(Vec::new()),
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Known(bounds(path)),
            available_actions: LocatorField::Known(actions),
            descriptors: Default::default(),
        },
    }
}

fn subtree(node: &Spec, path: &mut Vec<usize>, depth: u8) -> ObservedSubtree {
    let mut children = Vec::new();
    if depth > 0 {
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            children.push(subtree(child, path, depth - 1));
            path.pop();
        }
    }
    ObservedSubtree::new(evidence(node, path), children, true, None)
}

pub(crate) struct RootFindAdapter;

impl ObservationOps for RootFindAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let ui = ui();
        let (path, depth) = match &root {
            ObservationRoot::Window(_) => (Vec::new(), u8::MAX),
            ObservationRoot::Element { entry, .. } => (
                entry.scope.path.to_vec(),
                if request.max_logical_depth == 0 {
                    0
                } else {
                    u8::MAX
                },
            ),
        };
        let node = at(&ui, &path).ok_or_else(|| AdapterError::stale_ref_because("path is gone"))?;
        ObservedTree::from_roots(
            vec![subtree(node, &mut path.clone(), depth)],
            ObservationSource::from_root(&root, request.surface),
            LocatorStats::default(),
            true,
        )
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Fixture".into(),
            app: "FixtureApp".into(),
            pid: crate::ProcessId::new(101),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }

    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_locator_anchor(entry, deadline)
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let ui = ui();
        match at(&ui, &entry.scope.path) {
            Some(node) if node.role == entry.identity.role => {
                Ok(NativeHandle::new(entry.scope.path.to_vec()))
            }
            _ => Err(AdapterError::stale_ref_because("anchor is gone")),
        }
    }
}

impl ActionOps for RootFindAdapter {}
impl InputOps for RootFindAdapter {}
impl SystemOps for RootFindAdapter {
    fn supported_surfaces(&self) -> Vec<crate::SnapshotSurface> {
        vec![crate::SnapshotSurface::Window]
    }
}
