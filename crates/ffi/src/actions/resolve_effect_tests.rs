use super::test_ref_entry;
use crate::AdAdapter;
use crate::actions::execute::{ad_execute_action, ad_execute_ref_action_exact_with_policy};
use crate::actions::native_handle::{ad_free_handle, into_ffi_handle};
use crate::error::AdResult;
use crate::types::{
    AdAction, AdActionKind, AdActionResult, AdExactRefEntry, AdIdentifierKind, AdNativeHandle,
    AdPolicyKind,
};
use agent_desktop_core::{
    ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, InputOps, InteractionLease,
    NativeHandle, ObservationOps, ProcessIdentity, RefEntry, SystemOps,
};
use std::ffi::CString;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Default)]
struct Counters {
    resolves: AtomicU32,
    actions: AtomicU32,
}

struct EffectAdapter {
    counters: Arc<Counters>,
    refusal: Option<AdapterError>,
}

impl ObservationOps for EffectAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.counters.resolves.fetch_add(1, Ordering::SeqCst);
        match &self.refusal {
            Some(refusal) => Err(refusal.clone()),
            None => Ok(NativeHandle::null()),
        }
    }
}

impl ActionOps for EffectAdapter {
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

impl InputOps for EffectAdapter {}

impl SystemOps for EffectAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }
}

struct Registered {
    counters: Arc<Counters>,
    adapter: *mut AdAdapter,
}

impl Registered {
    fn new(refusal: Option<AdapterError>) -> Self {
        let counters = Arc::new(Counters::default());
        let adapter = crate::adapter::register_adapter(AdAdapter {
            inner: Box::new(EffectAdapter {
                counters: Arc::clone(&counters),
                refusal,
            }),
            session_id: None,
            _session_lease: None,
        })
        .unwrap();
        Self { counters, adapter }
    }

    fn resolves(&self) -> u32 {
        self.counters.resolves.load(Ordering::SeqCst)
    }

    fn actions(&self) -> u32 {
        self.counters.actions.load(Ordering::SeqCst)
    }
}

impl Drop for Registered {
    fn drop(&mut self) {
        unsafe { crate::adapter::ad_adapter_destroy(self.adapter) };
    }
}

fn click_action() -> AdAction {
    let mut action: AdAction = unsafe { std::mem::zeroed() };
    action.kind = AdActionKind::Click as i32;
    action
}

#[test]
fn a_refused_identity_never_dispatches_the_action() {
    let registered = Registered::new(Some(AdapterError::ambiguous_target(
        "Ambiguous target: 2 candidates matched",
    )));
    let role = CString::new("button").unwrap();
    let name = CString::new("Save").unwrap();
    let mut entry = test_ref_entry();
    entry.process.pid = 42;
    entry.identity.role = role.as_ptr();
    entry.identity.name = name.as_ptr();
    let exact = AdExactRefEntry {
        version: crate::types::exact_ref_entry::AD_EXACT_REF_ENTRY_VERSION,
        size: crate::types::exact_ref_entry::AD_EXACT_REF_ENTRY_SIZE as u32,
        entry,
        process_instance: c"42:100".as_ptr(),
        identifier_kind: AdIdentifierKind::AxIdentifier as i32,
    };
    let action = click_action();
    let mut out: AdActionResult = unsafe { std::mem::zeroed() };

    let result = unsafe {
        ad_execute_ref_action_exact_with_policy(
            registered.adapter,
            &exact,
            &action,
            AdPolicyKind::Headless as i32,
            &mut out,
        )
    };

    assert_eq!(result, AdResult::ErrAmbiguousTarget);
    assert_eq!(registered.resolves(), 1);
    assert_eq!(registered.actions(), 0);
}

#[test]
fn the_effect_counter_sees_a_delivered_action() {
    let registered = Registered::new(None);
    let owner = crate::adapter::adapter_id(registered.adapter).unwrap();
    let token = into_ffi_handle(
        owner,
        NativeHandle::null(),
        ProcessIdentity::new(42, "42:100"),
    )
    .unwrap();
    let mut handle = AdNativeHandle { ptr: token };
    let action = click_action();
    let mut out: AdActionResult = unsafe { std::mem::zeroed() };

    let result = unsafe { ad_execute_action(registered.adapter, &handle, &action, &mut out) };

    assert_eq!(result, AdResult::Ok);
    assert_eq!(registered.actions(), 1);
    unsafe { ad_free_handle(registered.adapter, &mut handle) };
}
