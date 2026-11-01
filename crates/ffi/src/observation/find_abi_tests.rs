use super::*;
use crate::adapter::AdAdapter;
use crate::types::{
    AdExactWindowInfo, AdFindQuery, AdFindSelectionKind, AdNativeHandle, AdRect, AdWindowInfo,
};
use agent_desktop_core::{ActionOps, InputOps, ObservationOps, SystemOps};
use agent_desktop_core::{
    IdentifierEvidence, LocatorEvidence, LocatorField, LocatorRefEvidence, ObservationRequest,
    ObservationRoot, ObservationSource, ObservedSubtree, ObservedTree,
};

const TEST_TIMEOUT_MS: u64 = 1_000;

struct CardinalityAdapter {
    duplicate: bool,
    complete: bool,
}

impl ActionOps for CardinalityAdapter {}
impl InputOps for CardinalityAdapter {}
impl SystemOps for CardinalityAdapter {}

impl ObservationOps for CardinalityAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, agent_desktop_core::AdapterError> {
        let ObservationRoot::Window(window) = root else {
            return Err(agent_desktop_core::AdapterError::internal(
                "expected window root",
            ));
        };
        let mut roots = vec![subtree(self.complete)];
        if self.duplicate {
            roots.push(subtree(self.complete));
        }
        ObservedTree::from_roots(
            roots,
            ObservationSource::Window {
                window: window.clone(),
                surface: agent_desktop_core::SnapshotSurface::Window,
            },
            Default::default(),
            self.complete,
        )
    }
}

fn subtree(complete: bool) -> ObservedSubtree {
    subtree_at(complete, LocatorField::Absent)
}

pub(super) fn subtree_at(
    complete: bool,
    bounds: LocatorField<agent_desktop_core::Rect>,
) -> ObservedSubtree {
    named_subtree_at("Save", complete, bounds)
}

pub(super) fn named_subtree_at(
    name: &str,
    complete: bool,
    bounds: LocatorField<agent_desktop_core::Rect>,
) -> ObservedSubtree {
    ObservedSubtree::new(
        LocatorEvidence {
            role: LocatorField::Known("button".into()),
            name: LocatorField::Known(name.into()),
            description: LocatorField::Absent,
            value: LocatorField::Absent,
            identifiers: IdentifierEvidence::absent(),
            states: LocatorField::Known(Vec::new()),
            ref_evidence: LocatorRefEvidence {
                window_bounds: LocatorField::Absent,
                bounds,
                available_actions: LocatorField::Known(vec!["Click".into()]),
                descriptors: Default::default(),
            },
        },
        Vec::new(),
        complete,
        None,
    )
}

pub(super) fn ffi_window() -> AdExactWindowInfo {
    AdExactWindowInfo {
        version: crate::types::exact_window_info::AD_EXACT_WINDOW_INFO_VERSION,
        size: crate::types::exact_window_info::AD_EXACT_WINDOW_INFO_SIZE as u32,
        window: AdWindowInfo {
            id: c"w-1".as_ptr(),
            title: c"Fixture".as_ptr(),
            app_name: c"Fixture".as_ptr(),
            pid: 42,
            bounds: AdRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            has_bounds: false,
            is_focused: false,
        },
        process_instance: c"42:100".as_ptr(),
        accessible: true,
    }
}

pub(super) fn query() -> AdFindQuery {
    let mut query = unsafe { std::mem::zeroed::<AdFindQuery>() };
    query.control.version = crate::types::find_query::AD_FIND_QUERY_VERSION;
    query.control.selection.kind = AdFindSelectionKind::Strict as i32;
    query.control.timeout_ms = TEST_TIMEOUT_MS;
    query.filter.identity.name = c"Save".as_ptr();
    query
}

fn call(adapter: CardinalityAdapter) -> AdResult {
    let handle = crate::adapter::register_adapter(AdAdapter {
        inner: Box::new(adapter),
        session_id: None,
        _session_lease: None,
    })
    .unwrap();
    let mut out = AdNativeHandle {
        ptr: std::ptr::null(),
    };
    let result = unsafe { ad_find_exact(handle, &ffi_window(), &query(), &mut out) };
    unsafe { crate::adapter::ad_adapter_destroy(handle) };
    assert!(out.ptr.is_null());
    result
}

#[test]
fn strict_duplicate_is_ambiguous_at_the_c_boundary() {
    assert_eq!(
        call(CardinalityAdapter {
            duplicate: true,
            complete: true,
        }),
        AdResult::ErrAmbiguousTarget
    );
}

#[test]
fn strict_incomplete_observation_times_out_at_the_c_boundary() {
    assert_eq!(
        call(CardinalityAdapter {
            duplicate: false,
            complete: false,
        }),
        AdResult::ErrTimeout
    );
}

struct ValueOnRequestAdapter {
    twins: usize,
    seen_basis: std::sync::Arc<std::sync::Mutex<Option<agent_desktop_core::IdentityBasis>>>,
}

impl ActionOps for ValueOnRequestAdapter {}
impl InputOps for ValueOnRequestAdapter {}
impl SystemOps for ValueOnRequestAdapter {}

impl ObservationOps for ValueOnRequestAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, agent_desktop_core::AdapterError> {
        let (window, twins) = match root {
            ObservationRoot::Window(window) => (window.clone(), self.twins),
            ObservationRoot::Element { .. } => {
                (crate::windows::ad_exact_window_to_core(&ffi_window())?, 1)
            }
        };
        let required = request.evidence_for_raw_depth(0);
        let read = |wanted: bool, value: &str| {
            if wanted {
                LocatorField::Known(value.to_owned())
            } else {
                LocatorField::Unknown
            }
        };
        let roots = (0..twins)
            .map(|_| {
                ObservedSubtree::new(
                    LocatorEvidence {
                        role: LocatorField::Known("textfield".into()),
                        name: LocatorField::Known("Address".into()),
                        description: LocatorField::Absent,
                        value: read(required.value, "https://a.test"),
                        identifiers: IdentifierEvidence::absent(),
                        states: LocatorField::Known(Vec::new()),
                        ref_evidence: LocatorRefEvidence {
                            window_bounds: LocatorField::Absent,
                            bounds: if required.ref_evidence.bounds {
                                LocatorField::Known(agent_desktop_core::Rect {
                                    x: 1.0,
                                    y: 2.0,
                                    width: 30.0,
                                    height: 10.0,
                                })
                            } else {
                                LocatorField::Unknown
                            },
                            available_actions: LocatorField::Known(vec!["SetValue".into()]),
                            descriptors: Default::default(),
                        },
                    },
                    Vec::new(),
                    true,
                    None,
                )
            })
            .collect();
        ObservedTree::from_roots(
            roots,
            ObservationSource::Window {
                window,
                surface: agent_desktop_core::SnapshotSurface::Window,
            },
            Default::default(),
            true,
        )
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &agent_desktop_core::RefEntry,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::NativeHandle, agent_desktop_core::AdapterError> {
        Ok(agent_desktop_core::NativeHandle::null())
    }

    fn resolve_element_strict(
        &self,
        entry: &agent_desktop_core::RefEntry,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::NativeHandle, agent_desktop_core::AdapterError> {
        *self.seen_basis.lock().unwrap() = entry.identity.basis;
        Err(agent_desktop_core::AdapterError::new(
            agent_desktop_core::ErrorCode::ElementNotFound,
            "strict resolution must not be used for a find the query proved unique",
        ))
    }
}

fn find_address(twins: usize) -> (AdResult, Option<agent_desktop_core::IdentityBasis>) {
    let seen_basis = std::sync::Arc::default();
    let handle = crate::adapter::register_adapter(AdAdapter {
        inner: Box::new(ValueOnRequestAdapter {
            twins,
            seen_basis: std::sync::Arc::clone(&seen_basis),
        }),
        session_id: None,
        _session_lease: None,
    })
    .unwrap();
    let mut find = query();
    find.filter.identity.name = c"Address".as_ptr();
    let mut out = AdNativeHandle {
        ptr: std::ptr::null(),
    };
    let result = unsafe { ad_find_exact(handle, &ffi_window(), &find, &mut out) };
    unsafe { crate::adapter::ad_adapter_destroy(handle) };
    let basis = *seen_basis.lock().unwrap();
    (result, basis)
}

#[test]
fn strict_find_of_a_unique_text_field_lands_through_the_verified_anchor() {
    assert_eq!(find_address(1), (AdResult::Ok, None));
}

#[test]
fn strict_find_of_twin_text_fields_stays_ambiguous() {
    assert_eq!(find_address(2), (AdResult::ErrAmbiguousTarget, None));
}
