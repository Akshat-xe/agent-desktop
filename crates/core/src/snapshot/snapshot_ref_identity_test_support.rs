use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree};
use crate::*;
use std::sync::Mutex;

pub(crate) struct DrillAdapter {
    pub tree: Mutex<AccessibilityNode>,
    pub actions: Mutex<Vec<Vec<usize>>>,
    pub replace_outer_on_target: std::sync::atomic::AtomicBool,
    pub hit_unknown: std::sync::atomic::AtomicBool,
    pub derived_names: std::sync::atomic::AtomicBool,
}

pub(crate) fn node(role: &str, name: &str) -> AccessibilityNode {
    let mut node = crate::adapter::minimal_accessibility_node(role, name);
    node.presentation.bounds = Some(Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    });
    node
}

pub(super) fn window() -> WindowInfo {
    WindowInfo {
        id: "w-42".into(),
        title: "Test".into(),
        app: "TestApp".into(),
        pid: ProcessId::new(42),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: Default::default(),
    }
}

impl DrillAdapter {
    pub fn new() -> Self {
        let mut root = node("group", "Sidebar");
        root.children.push(node("button", "Save"));
        let mut tree = node("window", "Test");
        tree.children = vec![root];
        Self {
            tree: Mutex::new(tree),
            actions: Mutex::new(Vec::new()),
            replace_outer_on_target: std::sync::atomic::AtomicBool::new(false),
            hit_unknown: std::sync::atomic::AtomicBool::new(false),
            derived_names: std::sync::atomic::AtomicBool::new(false),
        }
    }

    fn at(&self, path: &[usize]) -> Result<AccessibilityNode, AdapterError> {
        path.iter()
            .try_fold(self.tree.lock().unwrap().clone(), |node, index| {
                node.children.get(*index).cloned()
            })
            .ok_or_else(|| AdapterError::stale_ref_because("node removed"))
    }

    fn matches(entry: &RefEntry, node: &AccessibilityNode) -> bool {
        entry.identity.role == node.role
            && crate::refs::ref_identity::identity_match(
                entry,
                &LocatorField::Known(node.identity.name.clone().unwrap_or_default()),
                &LocatorField::Absent,
                &LocatorField::Absent,
                &IdentifierEvidence::absent(),
            ) == IdentityMatch::Match
    }
}

impl ObservationOps for DrillAdapter {
    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![window()])
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        fn prune(node: &mut AccessibilityNode, depth: u8, limit: u8) {
            if depth == limit {
                if !node.children.is_empty() {
                    node.children_count = Some(node.children.len() as u32);
                }
                node.children.clear();
            }
            for child in &mut node.children {
                prune(child, depth + 1, limit);
            }
        }
        let node = match root {
            ObservationRoot::Window(_) => {
                let mut tree = self.tree.lock().unwrap().clone();
                if request.observation_mode.skeleton {
                    prune(&mut tree, 0, request.max_logical_depth);
                }
                tree
            }
            ObservationRoot::Element { handle, .. } => {
                let mut node = self.at(handle.downcast_ref::<Vec<usize>>().unwrap())?;
                if self.derived_names.load(std::sync::atomic::Ordering::SeqCst)
                    && !request.observation_mode.skeleton
                    && node.role == "treeitem"
                {
                    node.identity.name = None;
                }
                node
            }
        };
        crate::adapter::observed_tree(&root, node)
    }

    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        crate::refs::identity_basis_gate::basis_gate(&entry.identity)?;
        fn collect(
            node: &AccessibilityNode,
            entry: &RefEntry,
            path: Vec<usize>,
            matches: &mut Vec<Vec<usize>>,
        ) {
            if DrillAdapter::matches(entry, node) {
                matches.push(path.clone());
            }
            for (index, child) in node.children.iter().enumerate() {
                let mut path = path.clone();
                path.push(index);
                collect(child, entry, path, matches);
            }
        }
        let mut matches = Vec::new();
        collect(&self.tree.lock().unwrap(), entry, Vec::new(), &mut matches);
        match matches.len() {
            0 => Err(AdapterError::stale_ref_because("no live match")
                .with_details(serde_json::json!({"complete": true}))),
            1 => Ok(NativeHandle::new(matches.remove(0))),
            _ => Err(AdapterError::ambiguous_target("several live matches")),
        }
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        let node = self.at(&entry.scope.path)?;
        if !Self::matches(entry, &node) {
            return Err(AdapterError::stale_ref_because("anchor changed"));
        }
        if entry.identity.role == "button"
            && self
                .replace_outer_on_target
                .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            let mut tree = self.tree.lock().unwrap();
            let mut outside = tree.children[0].clone();
            outside.identity.name = Some("Other".into());
            tree.children.insert(0, outside);
        }
        Ok(NativeHandle::new(entry.scope.path.to_vec()))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: Point,
        _deadline: Deadline,
    ) -> Result<crate::adapter::hit_test::HitTestResult, AdapterError> {
        Ok(
            if self.hit_unknown.load(std::sync::atomic::Ordering::SeqCst) {
                crate::adapter::hit_test::HitTestResult::Unknown
            } else {
                crate::adapter::hit_test::HitTestResult::ReachesTarget
            },
        )
    }

    fn confirm_pointer_receiver(
        &self,
        entry: &RefEntry,
        _point: Point,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        self.resolve_element_strict(entry, deadline).map(|_| ())
    }

    fn get_live_element(
        &self,
        handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        let node = self.at(handle.downcast_ref::<Vec<usize>>().unwrap())?;
        Ok(LiveElement {
            identity: crate::adapter::live_identity(node.identity.name.as_deref().unwrap_or("")),
            state: ElementState {
                role: node.role,
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: node.presentation.bounds,
            available_actions: vec!["Click".into()],
        })
    }

    fn get_element_bounds(
        &self,
        handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(self
            .at(handle.downcast_ref::<Vec<usize>>().unwrap())?
            .presentation
            .bounds)
    }
}

impl ActionOps for DrillAdapter {
    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.actions
            .lock()
            .unwrap()
            .push(handle.downcast_ref::<Vec<usize>>().unwrap().clone());
        Ok(ActionResult::delivered_unverified(request.action.name()))
    }
}

impl InputOps for DrillAdapter {}
impl SystemOps for DrillAdapter {
    crate::adapter::guarded_interaction_lease!();
}

pub(crate) fn save_map() -> (String, crate::refs::RefMap) {
    let mut map = crate::refs::RefMap::new();
    let mut root = crate::adapter::minimal_ref_entry("group", Some("Sidebar"));
    root.process.pid = ProcessId::new(42);
    root.source.source_app = Some("TestApp".into());
    root.source.source_window_id = Some("w-42".into());
    root.identity.basis = Some(IdentityBasis::Name);
    root.scope.path = smallvec::smallvec![0];
    map.allocate(root.clone());
    let mut target = root;
    target.identity.role = "button".into();
    target.identity.name = Some("Save".into());
    target.identity.basis = Some(IdentityBasis::Name);
    target.capabilities.available_actions = vec!["Click".into()];
    target.scope.root_ref = Some("@e1".into());
    target.scope.path_is_absolute = true;
    target.scope.path.push(0);
    map.allocate(target);
    let id = crate::refs::refs_store::RefStore::new()
        .unwrap()
        .save_new_snapshot(&map)
        .unwrap();
    (id, map)
}

pub(super) fn click(adapter: &DrillAdapter, token: &str) -> Result<ActionResult, AppError> {
    let (id, local) = crate::refs::ref_token::resolve_ref_target(token)?;
    let map = crate::refs::refs_store::RefStore::new()?.load_snapshot(&id)?;
    crate::ref_action::ref_action_wait::execute_with_auto_wait(
        crate::ref_action::ref_action_wait_context::RefActionWaitContext {
            adapter,
            entry: map.get(&local).unwrap(),
            ref_id: token,
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click),
        crate::ref_action::dispatch_resolved,
    )
    .map_err(AppError::Adapter)
}
