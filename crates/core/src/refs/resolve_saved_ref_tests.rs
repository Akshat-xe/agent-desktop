use std::sync::{
    Mutex,
    atomic::{AtomicU32, Ordering},
};

use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::locator::live_locator::ObservationRoot;
use crate::refs::resolve_saved_ref::resolve_saved_ref as resolve_entry;
use crate::{
    AccessibilityNode, AdapterError, Deadline, ErrorCode, NativeHandle, RefEntry, WindowInfo,
    refs::refs_test_support::HomeGuard,
};

struct RuleAdapter {
    windows: Vec<WindowInfo>,
    observed: AccessibilityNode,
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
            observed,
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
        crate::adapter::observed_tree(&root, self.observed.clone())
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.strict.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::new("strict"))
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
    entry.identity.unique_at_capture = true;
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
    resolve_entry(adapter, entry, Deadline::standard().unwrap())
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
    assert_eq!(adapter.counts(), (1, 1, 0, 1));
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
fn a_truncated_observation_never_resolves() {
    let _guard = HomeGuard::new();
    let mut truncated = tree_with(vec![node("button", "Run")]);
    truncated.subtree_truncated = true;
    let adapter = RuleAdapter::new(truncated);

    let Err(error) = resolve(&adapter, &entry()) else {
        panic!("a truncated walk is not evidence");
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.details.unwrap()["kind"],
        "resolution_identity_unknown"
    );
    assert_eq!(adapter.anchors.load(Ordering::SeqCst), 0);
}

fn predicate(entry: &RefEntry, role: &str, name: Option<&str>) -> crate::IdentityMatch {
    use crate::locator::live_locator::{
        IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence,
    };
    let field = |value: Option<&str>| {
        value
            .map(|value| LocatorField::Known(value.into()))
            .unwrap_or(LocatorField::Absent)
    };
    let evidence = LocatorEvidence {
        role: LocatorField::Known(role.into()),
        name: field(name),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: IdentifierEvidence::absent(),
        states: LocatorField::Absent,
        ref_evidence: LocatorRefEvidence {
            bounds: LocatorField::Absent,
            window_bounds: LocatorField::Absent,
            available_actions: LocatorField::Absent,
            descriptors: Default::default(),
        },
    };
    crate::refs::ref_identity::full_predicate_match(
        entry,
        crate::refs::ref_identity::PredicateEvidence::observed(&evidence),
    )
}

#[test]
fn the_full_predicate_requires_role_and_every_recorded_stable_field() {
    use crate::IdentityMatch;
    let entry = crate::adapter::minimal_ref_entry("button", Some("Save"));

    assert_eq!(
        predicate(&entry, "button", Some("Save")),
        IdentityMatch::Match
    );
    assert_eq!(
        predicate(&entry, "checkbox", Some("Save")),
        IdentityMatch::NoMatch
    );
    assert_eq!(
        predicate(&entry, "button", Some("Save as")),
        IdentityMatch::NoMatch
    );
    assert_eq!(predicate(&entry, "button", None), IdentityMatch::NoMatch);
}

#[test]
fn a_name_mirroring_a_mutable_value_is_not_stable_identity() {
    let mut entry = crate::adapter::minimal_ref_entry("textfield", Some("7"));
    entry.identity.value = Some("7".into());

    assert_eq!(
        predicate(&entry, "textfield", Some("7")),
        crate::IdentityMatch::Unknown
    );
}
