use crate::AdAdapter;
use crate::convert::string::optional_adapter_string;
use crate::convert::surface::snapshot_surface_from_c;
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;

use super::ref_input::{ref_path, string_array};
use crate::types::{AdExactRefEntry, AdIdentifierKind, AdNativeHandle, AdRefEntry};
use agent_desktop_core::{
    AdapterError, ElementIdentifier, ErrorCode, IdentityBasis, Rect, RefCapabilities,
    RefEntry as CoreRefEntry, RefEntryIdentity, RefGeometry, RefProcess, RefScope, RefSource,
};

const MAX_REF_FIELD_BYTES: usize = 65_536;
const MAX_REF_TOKEN_BYTES: usize = 256;

/// Resolves an element using process-generation and typed native-id evidence.
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
    if let Err(error) = agent_desktop_core::refs::identity_basis_gate::basis_gate(&entry.identity) {
        error::set_last_error(&error);
        return error::last_error_code();
    }
    match adapter.inner.resolve_element_strict(entry, deadline) {
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

pub(crate) unsafe fn core_ref_entry_from_exact(
    exact: &AdExactRefEntry,
) -> Result<CoreRefEntry, AdapterError> {
    if exact.version != crate::types::exact_ref_entry::AD_EXACT_REF_ENTRY_VERSION
        || exact.size as usize != crate::types::exact_ref_entry::AD_EXACT_REF_ENTRY_SIZE
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "AdExactRefEntry version or size does not match this library",
        ));
    }
    let process_instance = optional_adapter_string(exact.process_instance, "process_instance")?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AdapterError::new(ErrorCode::InvalidArgs, "process_instance is null or empty")
        })?;
    let native_id =
        match optional_adapter_string(exact.entry.identity.native_id, "identity.native_id")? {
            Some(value) if value.trim().is_empty() => {
                return Err(AdapterError::new(
                    ErrorCode::InvalidArgs,
                    "identity.native_id is empty",
                ));
            }
            Some(value) => {
                let kind = AdIdentifierKind::from_c(exact.identifier_kind).ok_or_else(|| {
                    AdapterError::new(
                        ErrorCode::InvalidArgs,
                        "identity.identifier_kind has an invalid discriminant",
                    )
                })?;
                Some(ElementIdentifier {
                    kind: kind.to_core(),
                    value,
                })
            }
            None => None,
        };
    unsafe { decode_core_ref_entry(&exact.entry, process_instance, native_id) }
}

unsafe fn decode_core_ref_entry(
    entry: &AdRefEntry,
    process_instance: String,
    native_id: Option<ElementIdentifier>,
) -> Result<CoreRefEntry, AdapterError> {
    if entry.process.pid == 0 {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "identity.pid must be positive",
        ));
    }
    let role = optional_adapter_string(entry.identity.role, "identity.role")?
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "role is null"))?;
    if !agent_desktop_core::Role::is_canonical(&role) || role == "unknown" {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "identity.role is not canonical",
        ));
    }
    let name = optional_adapter_string(entry.identity.name, "identity.name")?;
    let value = optional_adapter_string(entry.identity.value, "identity.value")?;
    let description = optional_adapter_string(entry.identity.description, "identity.description")?;
    let states = unsafe {
        string_array(
            entry.capabilities.states.items,
            entry.capabilities.states.count,
            "states",
            "AD_MAX_REF_STATES",
            crate::types::ref_entry::AD_MAX_REF_STATES,
        )
    }?;
    let available_actions = unsafe {
        string_array(
            entry.capabilities.available_actions.items,
            entry.capabilities.available_actions.count,
            "available_actions",
            "AD_MAX_REF_ACTIONS",
            crate::types::ref_entry::AD_MAX_REF_ACTIONS,
        )
    }?;
    let bounds = if entry.geometry.has_bounds {
        Some(
            Rect {
                x: entry.geometry.bounds.x,
                y: entry.geometry.bounds.y,
                width: entry.geometry.bounds.width,
                height: entry.geometry.bounds.height,
            }
            .validate()?,
        )
    } else {
        None
    };
    let supplied_bounds_hash = if entry.geometry.has_bounds_hash {
        Some(entry.geometry.bounds_hash)
    } else {
        None
    };
    let derived_bounds_hash = bounds.and_then(|value| value.bounds_hash());
    let source_surface = snapshot_surface_from_c(entry.source.surface, "source.surface")?;
    let path = unsafe { ref_path(entry.scope.path, entry.scope.path_count)? };
    if let (Some(supplied), Some(derived)) = (supplied_bounds_hash, derived_bounds_hash)
        && supplied != derived
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "geometry.bounds_hash does not match geometry.bounds",
        ));
    }
    let bounds_hash = supplied_bounds_hash.or(derived_bounds_hash);
    let source_app = optional_adapter_string(entry.source.app, "source.app")?;
    let source_window_id = optional_adapter_string(entry.source.window_id, "source.window_id")?;
    let source_window_title =
        optional_adapter_string(entry.source.window_title, "source.window_title")?;
    let source_window_bounds_hash = entry
        .source
        .has_window_bounds_hash
        .then_some(entry.source.window_bounds_hash);
    let root_ref = optional_adapter_string(entry.scope.root_ref, "scope.root_ref")?;
    for field in [
        Some(role.as_str()),
        Some(process_instance.as_str()),
        name.as_deref(),
        value.as_deref(),
        description.as_deref(),
        native_id
            .as_ref()
            .map(|identifier| identifier.value.as_str()),
        source_app.as_deref(),
        source_window_id.as_deref(),
        source_window_title.as_deref(),
        root_ref.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if field.len() > MAX_REF_FIELD_BYTES {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "ref identity evidence exceeds the field limit",
            ));
        }
    }
    if states
        .iter()
        .chain(available_actions.iter())
        .any(|value| value.len() > MAX_REF_TOKEN_BYTES)
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "ref state or action evidence exceeds the token limit",
        ));
    }

    let bounds_space = match entry.geometry.bounds_space {
        crate::AD_BOUNDS_SPACE_UNKNOWN => None,
        crate::AD_BOUNDS_SPACE_SCREEN => Some(agent_desktop_core::BoundsSpace::Screen),
        crate::AD_BOUNDS_SPACE_WINDOW => Some(agent_desktop_core::BoundsSpace::Window),
        _ => {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "invalid geometry.bounds_space",
            ));
        }
    };
    let required_space = if cfg!(target_os = "linux") {
        agent_desktop_core::BoundsSpace::Window
    } else {
        agent_desktop_core::BoundsSpace::Screen
    };
    if bounds_hash.is_some() && bounds_space != Some(required_space) {
        let frame = if cfg!(target_os = "linux") {
            "window"
        } else {
            "screen"
        };
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("Ref geometry requires {frame}-relative bounds on this platform"),
        ));
    }
    let decoded = CoreRefEntry {
        process: RefProcess {
            pid: agent_desktop_core::ProcessId::new(entry.process.pid),
            process_instance: Some(process_instance),
        },
        identity: RefEntryIdentity {
            basis: Some(identity_basis(
                native_id.as_ref(),
                agent_desktop_core::refs::ref_identity::stable_name(
                    role.as_str(),
                    name.as_deref(),
                    value.as_deref(),
                ),
                description.as_deref(),
            )),
            retained_object: None,
            role,
            name,
            value,
            description,
            native_id,
        },
        geometry: RefGeometry {
            bounds_space,
            bounds,
            bounds_hash,
        },
        capabilities: RefCapabilities {
            states,
            available_actions,
        },
        source: RefSource {
            source_app,
            source_window_id,
            source_window_title,
            source_window_bounds_hash,
            source_surface,
        },
        scope: RefScope {
            root_ref,
            path_is_absolute: entry.scope.path_is_absolute,
            census_depth: None,
            path,
        },
    };
    if !agent_desktop_core::refs::ref_identity::has_meaningful_identity(&decoded)
        && (bounds_space != Some(required_space) || bounds.is_none())
    {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("Geometry-only refs require {required_space:?}-relative bounds"),
        ));
    }
    Ok(decoded)
}

/// The basis a caller-supplied entry can prove on its own: the strongest non-blank identity
/// field it carries. Snapshot uniqueness is unprovable here, so the live search must find one match.
fn identity_basis(
    native_id: Option<&ElementIdentifier>,
    stable_name: Option<&str>,
    description: Option<&str>,
) -> IdentityBasis {
    if native_id.is_some_and(|identifier| !identifier.value.trim().is_empty()) {
        return IdentityBasis::NativeId;
    }
    if stable_name.is_some() {
        return IdentityBasis::Name;
    }
    if description.is_some_and(|value| !value.trim().is_empty()) {
        return IdentityBasis::Description;
    }
    IdentityBasis::Positional
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

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
