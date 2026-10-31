use super::super::resolve_core_entry;
use super::{decode, test_ref_entry};
use crate::AdAdapter;
use crate::actions::native_handle::ad_free_handle;
use crate::error::AdResult;
use crate::types::{AdDeliveryDisposition, AdDeliverySemantics, AdNativeHandle};
use agent_desktop_core::refs::identity_basis_gate::{
    basis_gate, bounds_may_decide, requires_live_uniqueness,
};
use agent_desktop_core::{
    ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, ErrorCode, IdentityBasis,
    InputOps, InteractionLease, NativeHandle, ObservationOps, RefCapabilities, RefEntry,
    RefEntryIdentity, RefGeometry, RefProcess, RefScope, RefSource, SnapshotSurface, SystemOps,
};
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Counters {
    resolves: AtomicU32,
    actions: AtomicU32,
}

struct StubAdapter {
    counters: Arc<Counters>,
    refusal: Option<AdapterError>,
    seen: Arc<Mutex<Option<RefEntry>>>,
}

impl ObservationOps for StubAdapter {
    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.counters.resolves.fetch_add(1, Ordering::SeqCst);
        *self.seen.lock().unwrap() = Some(entry.clone());
        match &self.refusal {
            Some(refusal) => Err(refusal.clone()),
            None => Ok(NativeHandle::null()),
        }
    }
}

impl ActionOps for StubAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        self.counters.actions.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified(request.action.name()))
    }
}

impl InputOps for StubAdapter {}

impl SystemOps for StubAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }
}

struct Registered {
    counters: Arc<Counters>,
    seen: Arc<Mutex<Option<RefEntry>>>,
    adapter: *mut AdAdapter,
}

impl Registered {
    fn new(refusal: Option<AdapterError>) -> Self {
        let counters = Arc::new(Counters::default());
        let seen = Arc::new(Mutex::new(None));
        let adapter = crate::adapter::register_adapter(AdAdapter {
            inner: Box::new(StubAdapter {
                counters: Arc::clone(&counters),
                refusal,
                seen: Arc::clone(&seen),
            }),
            session_id: None,
            _session_lease: None,
        })
        .unwrap();
        Self {
            counters,
            seen,
            adapter,
        }
    }

    fn resolves(&self) -> u32 {
        self.counters.resolves.load(Ordering::SeqCst)
    }

    fn actions(&self) -> u32 {
        self.counters.actions.load(Ordering::SeqCst)
    }

    fn seen(&self) -> Option<RefEntry> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for Registered {
    fn drop(&mut self) {
        unsafe { crate::adapter::ad_adapter_destroy(self.adapter) };
    }
}

fn core_entry(basis: Option<IdentityBasis>) -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: agent_desktop_core::ProcessId::new(42),
            process_instance: Some("42:100".into()),
        },
        identity: RefEntryIdentity {
            basis,
            retained_object: None,
            role: "button".into(),
            name: Some("Save".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: Default::default(),
        },
    }
}

fn native_bounds_space() -> i32 {
    if cfg!(target_os = "linux") {
        crate::AD_BOUNDS_SPACE_WINDOW
    } else {
        crate::AD_BOUNDS_SPACE_SCREEN
    }
}

fn delivery_semantics() -> AdDeliverySemantics {
    let mut semantics: AdDeliverySemantics = unsafe { std::mem::zeroed() };
    let result = unsafe { crate::error::ad_last_error_delivery_semantics(&mut semantics) };
    assert_eq!(result, AdResult::Ok);
    semantics
}

fn last_error_details() -> serde_json::Value {
    let pointer = crate::error::ad_last_error_details();
    assert!(!pointer.is_null(), "refusal must carry structured details");
    serde_json::from_str(unsafe { CStr::from_ptr(pointer) }.to_str().unwrap()).unwrap()
}

fn resolve(registered: &Registered, entry: &RefEntry) -> (AdResult, AdNativeHandle) {
    let mut out = AdNativeHandle {
        ptr: std::ptr::null(),
    };
    let result = unsafe { resolve_core_entry(registered.adapter, entry, &mut out) };
    (result, out)
}

#[test]
fn a_legacy_entry_is_refused_before_the_adapter_searches() {
    let registered = Registered::new(None);

    let (result, out) = resolve(&registered, &core_entry(None));

    assert_eq!(result, AdResult::ErrStaleRef);
    assert!(out.ptr.is_null());
    assert_eq!(registered.resolves(), 0);
    assert_eq!(registered.actions(), 0);
    assert_eq!(
        delivery_semantics().delivery,
        AdDeliveryDisposition::NotDelivered as i32
    );
}

#[test]
fn an_ambiguous_entry_is_refused_before_the_adapter_searches() {
    let registered = Registered::new(None);

    let (result, out) = resolve(&registered, &core_entry(Some(IdentityBasis::Ambiguous)));

    assert_eq!(result, AdResult::ErrAmbiguousTarget);
    assert!(out.ptr.is_null());
    assert_eq!(registered.resolves(), 0);
    assert_eq!(registered.actions(), 0);
    assert_eq!(last_error_details()["kind"], "identity_not_unique");
    assert_eq!(
        delivery_semantics().delivery,
        AdDeliveryDisposition::NotDelivered as i32
    );
}

#[test]
fn an_unreadable_subtree_after_a_match_never_yields_a_handle() {
    let refusal = AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Strict element resolution observed an incomplete accessibility tree during depth_limit",
    )
    .with_details(serde_json::json!({
        "kind": "resolution_traversal_incomplete",
        "phase": "depth_limit",
        "complete": false,
        "retryable": true,
    }));
    let registered = Registered::new(Some(refusal));

    let (result, out) = resolve(&registered, &core_entry(Some(IdentityBasis::Name)));

    assert_eq!(result, AdResult::ErrAppUnresponsive);
    assert!(out.ptr.is_null());
    assert_eq!(registered.resolves(), 1);
    assert_eq!(registered.actions(), 0);
}

#[test]
fn a_timed_out_live_count_never_yields_a_handle() {
    let refusal = AdapterError::new(
        ErrorCode::Timeout,
        "Strict resolution could not determine candidate identity from the live accessibility evidence",
    )
    .with_details(serde_json::json!({
        "kind": "resolution_identity_unknown",
        "complete": false,
        "retryable": true,
    }));
    let registered = Registered::new(Some(refusal));

    let (result, out) = resolve(&registered, &core_entry(Some(IdentityBasis::Name)));

    assert_eq!(result, AdResult::ErrTimeout);
    assert!(out.ptr.is_null());
    assert_eq!(registered.resolves(), 1);
    assert_eq!(registered.actions(), 0);
}

/// The C struct carries no basis field, so `identity_basis` derives one from
/// the entry's own fields: native identifier, then name, then description, and
/// positional when none is present. It can never derive `Ambiguous` or `None`,
/// so a caller cannot claim a weaker basis than its evidence, and every derived
/// basis other than positional sends the adapter into its live uniqueness count.
#[test]
fn a_derived_basis_always_requires_the_live_uniqueness_count() {
    let bounds = crate::types::AdRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    };
    let native = CString::new("row-7").unwrap();
    let name = CString::new("Save").unwrap();
    let description = CString::new("Commits changes").unwrap();
    let value = CString::new("On").unwrap();
    let shapes = [
        (Some(&native), None, None, None, IdentityBasis::NativeId),
        (
            Some(&native),
            Some(&name),
            None,
            None,
            IdentityBasis::NativeId,
        ),
        (None, Some(&name), None, None, IdentityBasis::Name),
        (
            None,
            None,
            Some(&description),
            None,
            IdentityBasis::Description,
        ),
        (
            None,
            Some(&name),
            Some(&description),
            None,
            IdentityBasis::Name,
        ),
        (None, None, None, Some(&value), IdentityBasis::Positional),
        (None, None, None, None, IdentityBasis::Positional),
    ];

    for (native_id, shape_name, shape_description, shape_value, expected_basis) in shapes {
        let mut entry = test_ref_entry();
        entry.identity.role = c"button".as_ptr();
        entry.identity.native_id = native_id.map_or(std::ptr::null(), |value| value.as_ptr());
        entry.identity.name = shape_name.map_or(std::ptr::null(), |value| value.as_ptr());
        entry.identity.description =
            shape_description.map_or(std::ptr::null(), |value| value.as_ptr());
        entry.identity.value = shape_value.map_or(std::ptr::null(), |value| value.as_ptr());
        entry.geometry.bounds = bounds;
        entry.geometry.has_bounds = true;
        entry.geometry.bounds_space = native_bounds_space();
        let registered = Registered::new(None);
        let decoded = decode(entry).unwrap();

        assert_eq!(decoded.identity.basis, Some(expected_basis));
        assert!(basis_gate(&decoded.identity).is_ok());
        if expected_basis == IdentityBasis::Positional {
            assert!(decoded.identity.native_id.is_none());
            assert!(decoded.identity.name.is_none());
            assert!(decoded.identity.description.is_none());
            assert!(!requires_live_uniqueness(&decoded.identity));
            assert!(bounds_may_decide(&decoded.identity));
        } else {
            assert!(requires_live_uniqueness(&decoded.identity));
            assert!(!bounds_may_decide(&decoded.identity));
        }

        let (result, mut out) = resolve(&registered, &decoded);
        assert_eq!(result, AdResult::Ok);
        let seen = registered
            .seen()
            .expect("the adapter must receive the entry");
        assert_eq!(seen.identity.basis, Some(expected_basis));
        assert_eq!(
            requires_live_uniqueness(&seen.identity),
            expected_basis != IdentityBasis::Positional
        );
        unsafe { ad_free_handle(registered.adapter, &mut out) };
    }
}

#[test]
fn the_c_entry_cannot_supply_a_retained_object_token() {
    let mut named = test_ref_entry();
    named.identity.role = c"button".as_ptr();
    named.identity.name = c"Save".as_ptr();
    let decoded = decode(named).unwrap();
    assert!(decoded.identity.retained_object.is_none());

    let mut identified = test_ref_entry();
    identified.identity.role = c"button".as_ptr();
    identified.identity.native_id = c"row-7".as_ptr();
    let decoded = decode(identified).unwrap();
    assert!(decoded.identity.retained_object.is_none());
}

#[test]
fn a_positional_entry_keeps_its_path_and_bounds() {
    let registered = Registered::new(None);
    let path = [2_u32, 0];
    let mut entry = test_ref_entry();
    entry.identity.role = c"button".as_ptr();
    entry.geometry.bounds = crate::types::AdRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    };
    entry.geometry.has_bounds = true;
    entry.geometry.bounds_space = native_bounds_space();
    entry.scope.path = path.as_ptr();
    entry.scope.path_count = path.len();
    entry.scope.path_is_absolute = true;

    let decoded = decode(entry).unwrap();
    assert_eq!(decoded.identity.basis, Some(IdentityBasis::Positional));
    let bounds_hash = decoded
        .geometry
        .bounds_hash
        .expect("a positional ref keeps its geometry");

    let (result, mut out) = resolve(&registered, &decoded);

    assert_eq!(result, AdResult::Ok);
    assert!(!out.ptr.is_null());
    let seen = registered
        .seen()
        .expect("the adapter must receive the entry");
    assert_eq!(seen.identity.basis, Some(IdentityBasis::Positional));
    assert_eq!(seen.scope.path.as_slice(), &[2_usize, 0]);
    assert!(seen.scope.path_is_absolute);
    assert_eq!(seen.geometry.bounds_hash, Some(bounds_hash));
    unsafe { ad_free_handle(registered.adapter, &mut out) };
}
