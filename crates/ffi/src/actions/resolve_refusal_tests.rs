use super::super::resolve_core_entry;
use super::entry_tests::core_entry;
use super::{core_ref_entry_from_exact, test_ref_entry};
use crate::AdAdapter;
use crate::actions::native_handle::ad_free_handle;
use crate::error::AdResult;
use crate::types::exact_ref_entry::{AD_EXACT_REF_ENTRY_SIZE, AD_EXACT_REF_ENTRY_VERSION};
use crate::types::{
    AdDeliveryDisposition, AdDeliverySemantics, AdExactRefEntry, AdIdentifierKind, AdNativeHandle,
    AdRefEntry,
};
use agent_desktop_core::{
    ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, ErrorCode, IdentityBasis,
    InputOps, InteractionLease, NativeHandle, ObservationOps, RefEntry, SystemOps,
};
use std::ffi::CStr;
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

pub(super) struct Registered {
    counters: Arc<Counters>,
    seen: Arc<Mutex<Option<RefEntry>>>,
    pub(super) adapter: *mut AdAdapter,
}

impl Registered {
    pub(super) fn new(refusal: Option<AdapterError>) -> Self {
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

    pub(super) fn resolves(&self) -> u32 {
        self.counters.resolves.load(Ordering::SeqCst)
    }

    pub(super) fn actions(&self) -> u32 {
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

fn exact_of(mut entry: AdRefEntry) -> AdExactRefEntry {
    entry.process.pid = 42;
    AdExactRefEntry {
        version: AD_EXACT_REF_ENTRY_VERSION,
        size: AD_EXACT_REF_ENTRY_SIZE as u32,
        entry,
        process_instance: c"42:100".as_ptr(),
        identifier_kind: AdIdentifierKind::AxIdentifier as i32,
    }
}

fn resolve_exact(registered: &Registered, exact: &AdExactRefEntry) -> (AdResult, AdNativeHandle) {
    let mut out = AdNativeHandle {
        ptr: std::ptr::null(),
    };
    let result = unsafe {
        crate::actions::resolve::ad_resolve_element_exact(registered.adapter, exact, &mut out)
    };
    (result, out)
}

/// Only a native id proves uniqueness for a C caller, so name or description evidence alone
/// derives `Ambiguous` and is refused before resolution; no evidence at all stays positional.
#[test]
fn exact_ref_derivation_trusts_only_native_ids() {
    let bounds = crate::types::AdRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    };
    let shapes = [
        (c"row-7", c"", c"", IdentityBasis::NativeId),
        (c"row-7", c"Save", c"", IdentityBasis::NativeId),
        (c"", c"Save", c"", IdentityBasis::Ambiguous),
        (c"", c"", c"Commits changes", IdentityBasis::Ambiguous),
        (c"", c"Save", c"Commits changes", IdentityBasis::Ambiguous),
        (c"", c"", c"", IdentityBasis::Positional),
    ];
    for (native_id, name, description, expected_basis) in shapes {
        let pick = |value: &'static CStr| {
            if value.is_empty() {
                std::ptr::null()
            } else {
                value.as_ptr()
            }
        };
        let mut entry = test_ref_entry();
        entry.identity.role = c"button".as_ptr();
        entry.identity.native_id = pick(native_id);
        entry.identity.name = pick(name);
        entry.identity.description = pick(description);
        entry.geometry.bounds = bounds;
        entry.geometry.has_bounds = true;
        entry.geometry.bounds_space = native_bounds_space();
        let registered = Registered::new(None);
        let exact = exact_of(entry);
        let decoded = unsafe { core_ref_entry_from_exact(&exact) }.unwrap();
        assert_eq!(decoded.identity.basis, Some(expected_basis));

        let (result, mut out) = resolve_exact(&registered, &exact);

        if expected_basis == IdentityBasis::Ambiguous {
            assert_eq!(result, AdResult::ErrAmbiguousTarget);
            assert!(out.ptr.is_null());
            assert_eq!(registered.resolves(), 0);
            continue;
        }
        assert_eq!(result, AdResult::Ok);
        let seen = registered.seen().unwrap();
        assert_eq!(seen.identity.basis, Some(expected_basis));
        assert_eq!(registered.actions(), 0);
        unsafe { ad_free_handle(registered.adapter, &mut out) };
    }
}

#[test]
fn invalid_identity_or_scope_is_refused_before_resolution() {
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

    let mut scoped = test_ref_entry();
    scoped.identity.role = c"button".as_ptr();
    scoped.identity.native_id = c"row-7".as_ptr();
    scoped.scope.root_ref = c"@e1".as_ptr();
    let (result, out) = resolve_exact(&registered, &exact_of(scoped));

    assert_eq!(result, AdResult::ErrStaleRef);
    assert!(out.ptr.is_null());
    assert_eq!(registered.resolves(), 0);
    assert_eq!(registered.actions(), 0);
}

#[test]
fn exact_resolver_preserves_live_refusal() {
    let refusals = [
        (
            AdapterError::new(
                ErrorCode::AppUnresponsive,
                "Strict element resolution observed an incomplete accessibility tree during depth_limit",
            )
            .with_details(serde_json::json!({
                "kind": "resolution_traversal_incomplete",
                "phase": "depth_limit",
                "complete": false,
                "retryable": true,
            })),
            AdResult::ErrAppUnresponsive,
        ),
        (
            AdapterError::new(
                ErrorCode::Timeout,
                "Strict resolution could not determine candidate identity from the live accessibility evidence",
            )
            .with_details(serde_json::json!({
                "kind": "resolution_identity_unknown",
                "complete": false,
                "retryable": true,
            })),
            AdResult::ErrTimeout,
        ),
    ];

    for (refusal, expected) in refusals {
        let registered = Registered::new(Some(refusal));
        let mut entry = test_ref_entry();
        entry.identity.role = c"button".as_ptr();
        entry.identity.native_id = c"row-7".as_ptr();

        let (result, out) = resolve_exact(&registered, &exact_of(entry));

        assert_eq!(result, expected);
        assert!(out.ptr.is_null());
        assert_eq!(registered.resolves(), 1);
        assert_eq!(registered.actions(), 0);
    }
}
