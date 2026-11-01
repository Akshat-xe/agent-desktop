use std::sync::{
    Mutex,
    atomic::{AtomicU32, Ordering},
};

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::ObservationRoot;
use crate::refs::ref_alloc::allocate_refs;
use crate::refs::ref_alloc_config::RefAllocConfig;
use crate::refs::ref_alloc_options::RefAllocOptions;
use crate::refs::ref_alloc_scope::RefAllocScope;
use crate::refs::ref_alloc_source::RefAllocSource;
use crate::refs::resolve_saved_ref::resolve_saved_ref as resolve_entry;
use crate::{
    AccessibilityNode, AdapterError, Deadline, ErrorCode, NativeHandle, RefEntry, RefMap,
    WindowInfo, refs::refs_test_support::HomeGuard,
};

struct RuleAdapter {
    windows: Vec<WindowInfo>,
    shell: Option<WindowInfo>,
    observed: AccessibilityNode,
    landed: Option<AccessibilityNode>,
    windows_listed: AtomicU32,
    observations: AtomicU32,
    strict: AtomicU32,
    anchors: AtomicU32,
    anchored: Mutex<Option<RefEntry>>,
}

impl RuleAdapter {
    fn new(observed: AccessibilityNode) -> Self {
        Self {
            windows: vec![window()],
            shell: None,
            observed,
            landed: None,
            windows_listed: AtomicU32::new(0),
            observations: AtomicU32::new(0),
            strict: AtomicU32::new(0),
            anchors: AtomicU32::new(0),
            anchored: Mutex::new(None),
        }
    }

    fn counts(&self) -> (u32, u32, u32, u32) {
        (
            self.windows_listed.load(Ordering::SeqCst),
            self.observations.load(Ordering::SeqCst),
            self.strict.load(Ordering::SeqCst),
            self.anchors.load(Ordering::SeqCst),
        )
    }
}

impl ObservationOps for RuleAdapter {
    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        self.windows_listed.fetch_add(1, Ordering::SeqCst);
        Ok(self.windows.clone())
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        self.observations.fetch_add(1, Ordering::SeqCst);
        match (&self.landed, root) {
            (Some(landed), ObservationRoot::Element { .. }) => {
                crate::adapter::observed_tree(&root, landed.clone())
            }
            (None, ObservationRoot::Element { .. }) => crate::adapter::saved_ref_element_tree(root),
            _ => crate::adapter::observed_tree(&root, self.observed.clone()),
        }
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.strict.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::new("strict"))
    }

    fn resolve_shell_surface(
        &self,
        _surface: crate::SnapshotSurface,
        _deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        match &self.shell {
            Some(window) => Ok(window.clone()),
            None => Err(AdapterError::not_supported("resolve_shell_surface")),
        }
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.anchors.fetch_add(1, Ordering::SeqCst);
        *self.anchored.lock().unwrap() = Some(entry.clone());
        Ok(NativeHandle::new("anchor"))
    }
}

impl ActionOps for RuleAdapter {}
impl InputOps for RuleAdapter {}
impl SystemOps for RuleAdapter {}

fn window() -> WindowInfo {
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

fn entry() -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    entry.source.source_app = Some("Fixture".into());
    entry.source.source_window_id = Some("w-1".into());
    entry
}

fn node(role: &str, name: &str) -> AccessibilityNode {
    crate::adapter::minimal_accessibility_node(role, name)
}

fn tree_with(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    let mut root = node("group", "Window");
    root.children = children;
    root
}

fn resolve(adapter: &RuleAdapter, entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    resolve_entry(adapter, entry, Deadline::standard().unwrap(), None)
}

fn capture(root: AccessibilityNode, name: &str) -> RefEntry {
    let mut refmap = RefMap::new();
    let observed = crate::adapter::observed_tree(&ObservationRoot::Window(&window()), root.clone())
        .expect("the fixture observes");
    let unique = crate::refs::ref_alloc_unique::unique_capture_paths(&observed);
    let config = RefAllocConfig {
        unique_entries: Some(&unique),
        ..config_without_uniqueness()
    };
    allocate_refs(root, &mut refmap, &config).unwrap();
    (1..=refmap.len())
        .filter_map(|index| refmap.get(&format!("@e{index}")).cloned())
        .find(|entry| entry.identity.name.as_deref() == Some(name))
        .expect("the captured element has a ref")
}

fn config_without_uniqueness() -> RefAllocConfig<'static> {
    RefAllocConfig {
        options: RefAllocOptions {
            include_bounds: false,
            interactive_only: false,
            compact: false,
        },
        source: RefAllocSource {
            pid: crate::ProcessId::new(1),
            app: Some("Fixture"),
            window_id: Some("w-1"),
            window_title: Some("Fixture"),
            window_bounds_hash: None,
            process_instance: Some("test-instance"),
            surface: crate::SnapshotSurface::Window,
        },
        scope: RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
        unique_entries: None,
    }
}

#[test]
fn a_unique_live_match_resolves_through_a_positional_anchor() {
    let _guard = HomeGuard::new();
    let adapter = RuleAdapter::new(tree_with(vec![
        node("button", "Cancel"),
        node("button", "Run"),
    ]));

    let Ok(handle) = resolve(&adapter, &entry()) else {
        panic!("one live match must resolve");
    };

    assert_eq!(handle.downcast_ref::<&str>(), Some(&"anchor"));
    assert_eq!(adapter.counts(), (1, 2, 0, 1));
    let anchor = adapter
        .anchored
        .lock()
        .unwrap()
        .clone()
        .expect("anchor entry");
    assert_eq!(anchor.scope.path.as_slice(), [1]);
    assert_eq!(anchor.identity.name.as_deref(), Some("Run"));
}

#[test]
fn a_live_twin_refuses_as_ambiguous() {
    let _guard = HomeGuard::new();
    let adapter = RuleAdapter::new(tree_with(vec![
        node("button", "Run"),
        node("button", "Run"),
    ]));

    let Err(error) = resolve(&adapter, &entry()) else {
        panic!("a twin cannot be picked");
    };

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(error.details.unwrap()["kind"], "saved_ref_ambiguous");
    assert_eq!(adapter.anchors.load(Ordering::SeqCst), 0);
}

#[test]
fn a_landed_element_that_is_not_the_recorded_one_is_stale() {
    let _guard = HomeGuard::new();
    let mut adapter = RuleAdapter::new(tree_with(vec![node("button", "Run")]));
    adapter.landed = Some(node("button", "Cancel"));

    let Err(error) = resolve(&adapter, &entry()) else {
        panic!("the anchor landed a different element");
    };

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.details.unwrap()["kind"], "saved_ref_absent");
    assert_eq!(adapter.anchors.load(Ordering::SeqCst), 1);
}

/// A shell container is not in the window inventory, so the recorded window id
/// finds it only through the adapter's shell-surface seam.
#[test]
fn a_shell_surface_ref_reaches_its_container_without_the_window_inventory() {
    let _guard = HomeGuard::new();
    let mut adapter = RuleAdapter::new(tree_with(vec![node("button", "Run")]));
    adapter.windows.clear();
    adapter.shell = Some(window());
    let mut entry = entry();
    entry.source.source_window_id = Some("w-1".into());
    entry.source.source_surface = crate::SnapshotSurface::SystemTray;

    let Ok(handle) = resolve(&adapter, &entry) else {
        panic!("the shell container answers for its recorded id");
    };

    assert_eq!(handle.downcast_ref::<&str>(), Some(&"anchor"));
    assert_eq!(adapter.windows_listed.load(Ordering::SeqCst), 0);
}

#[test]
fn a_removed_target_is_stale() {
    let _guard = HomeGuard::new();
    let adapter = RuleAdapter::new(tree_with(vec![node("button", "Cancel")]));

    let Err(error) = resolve(&adapter, &entry()) else {
        panic!("a removed target is stale");
    };

    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.unwrap();
    assert_eq!(details["kind"], "saved_ref_absent");
    assert_eq!(details["complete"], true);
    assert_eq!(adapter.anchors.load(Ordering::SeqCst), 0);
}

#[test]
fn an_entry_not_unique_at_capture_is_refused_before_any_live_search() {
    let _guard = HomeGuard::new();
    let adapter = RuleAdapter::new(tree_with(vec![node("button", "Run")]));
    let mut entry = entry();
    entry.identity.unique_at_capture = false;

    let Err(error) = resolve(&adapter, &entry) else {
        panic!("an unproven ref must be refused");
    };

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(error.details.unwrap()["kind"], "identity_not_unique");
    assert_eq!(adapter.counts(), (0, 0, 0, 0));
}

#[test]
fn an_element_token_goes_straight_to_the_adapter() {
    let _guard = HomeGuard::new();
    let adapter = RuleAdapter::new(tree_with(vec![]));
    let mut entry = entry();
    entry.identity.unique_at_capture = false;
    entry.identity.retained_object = Some("axtok1:page-7:node-3".into());

    let Ok(handle) = resolve(&adapter, &entry) else {
        panic!("a token is re-identified by the adapter");
    };

    assert_eq!(handle.downcast_ref::<&str>(), Some(&"strict"));
    assert_eq!(adapter.counts(), (0, 0, 1, 0));
}

#[test]
fn incomplete_evidence_never_resolves() {
    let _guard = HomeGuard::new();
    let mut truncated = node("scrollarea", "Feed");
    truncated.children_count = Some(2);
    let adapter = RuleAdapter::new(tree_with(vec![node("button", "Run"), truncated]));

    let Err(error) = resolve(&adapter, &entry()) else {
        panic!("an unread subtree hides a twin");
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.details.unwrap()["kind"],
        "resolution_identity_unknown"
    );
    assert_eq!(adapter.anchors.load(Ordering::SeqCst), 0);
}

#[test]
fn capture_and_resolution_agree_on_the_same_element() {
    let _guard = HomeGuard::new();
    let alone = tree_with(vec![node("button", "Run"), node("button", "Cancel")]);
    let captured = capture(alone.clone(), "Run");
    assert!(captured.identity.unique_at_capture);
    assert!(resolve(&RuleAdapter::new(alone), &captured).is_ok());

    let twinned = tree_with(vec![node("button", "Run"), node("button", "Run")]);
    let twin_captured = capture(twinned.clone(), "Run");
    assert!(!twin_captured.identity.unique_at_capture);
    let refused = resolve(&RuleAdapter::new(twinned), &twin_captured);
    assert_eq!(
        refused.err().map(|error| error.code),
        Some(ErrorCode::AmbiguousTarget)
    );

    let renamed = tree_with(vec![node("button", "Cancel")]);
    let stale = resolve(&RuleAdapter::new(renamed), &captured);
    assert_eq!(
        stale.err().map(|error| error.code),
        Some(ErrorCode::StaleRef)
    );
}
