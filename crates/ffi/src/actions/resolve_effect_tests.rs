use super::refusal_tests::Registered;
use super::test_ref_entry;
use crate::actions::execute::{ad_execute_action, ad_execute_ref_action_exact_with_policy};
use crate::actions::native_handle::{ad_free_handle, into_ffi_handle};
use crate::error::AdResult;
use crate::types::{
    AdAction, AdActionKind, AdActionResult, AdExactRefEntry, AdIdentifierKind, AdNativeHandle,
    AdPolicyKind,
};
use agent_desktop_core::{AdapterError, NativeHandle, ProcessIdentity};
use std::ffi::CString;

fn click_action() -> AdAction {
    let mut action: AdAction = unsafe { std::mem::zeroed() };
    action.kind = AdActionKind::Click as i32;
    action
}

#[test]
fn refusal_prevents_dispatch_with_a_working_effect_counter() {
    let registered = Registered::new(Some(AdapterError::ambiguous_target(
        "Ambiguous target: 2 candidates matched",
    )));
    let role = CString::new("button").unwrap();
    let native_id = CString::new("save-button").unwrap();
    let mut entry = test_ref_entry();
    entry.process.pid = 42;
    entry.identity.role = role.as_ptr();
    entry.identity.native_id = native_id.as_ptr();
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
