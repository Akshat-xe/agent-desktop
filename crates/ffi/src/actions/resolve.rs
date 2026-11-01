pub(crate) use crate::actions::exact_entry::core_ref_entry_from_exact;

use crate::AdAdapter;
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdExactRefEntry, AdNativeHandle};
use agent_desktop_core::{AdapterError, ErrorCode, RefEntry as CoreRefEntry};

/// Resolves an exact entry, which carries no retained element token and no
/// capture proof: it fails with `AMBIGUOUS_TARGET` (`identity_not_unique`), so
/// callers act through a snapshot ref instead.
///
/// # Safety
///
/// `adapter` and `entry` must be live and valid; `out` must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_resolve_element_exact(
    adapter: *const AdAdapter,
    entry: *const AdExactRefEntry,
    out: *mut AdNativeHandle,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        (*out).ptr = std::ptr::null();
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(entry, c"entry is null");
        let core_entry = match core_ref_entry_from_exact(&*entry) {
            Ok(entry) => entry,
            Err(error) => {
                error::set_last_error(&error);
                return error::last_error_code();
            }
        };
        resolve_core_entry(adapter, &core_entry, out)
    })
}

unsafe fn resolve_core_entry(
    adapter: *const AdAdapter,
    entry: &CoreRefEntry,
    out: *mut AdNativeHandle,
) -> AdResult {
    let adapter_id = adapter.addr();
    let adapter = crate::adapter::acquire_adapter!(adapter);
    let process = match process_identity(entry) {
        Ok(process) => process,
        Err(error) => {
            error::set_last_error(&error);
            return error::last_error_code();
        }
    };
    let deadline = crate::operation::operation_deadline!();
    let resolved = agent_desktop_core::refs::resolve_saved_ref::resolve_saved_ref(
        adapter.inner.as_ref(),
        entry,
        deadline,
    );
    match resolved {
        Ok(handle) => {
            match crate::actions::native_handle::into_ffi_handle(adapter_id, handle, process) {
                Ok(token) => {
                    unsafe { (*out).ptr = token };
                    AdResult::Ok
                }
                Err(error) => {
                    error::set_last_error(&error);
                    error::last_error_code()
                }
            }
        }
        Err(error) => {
            error::set_last_error(&error);
            error::last_error_code()
        }
    }
}

fn process_identity(
    entry: &CoreRefEntry,
) -> Result<agent_desktop_core::ProcessIdentity, AdapterError> {
    let instance = entry.process.process_instance.clone().ok_or_else(|| {
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "process_instance is required for a native handle",
        )
    })?;
    Ok(agent_desktop_core::ProcessIdentity::new(
        entry.process.pid,
        instance,
    ))
}
