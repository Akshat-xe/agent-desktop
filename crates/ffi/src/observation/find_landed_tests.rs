use super::abi_tests::{ffi_window, named_subtree_at, query, subtree_at};
use super::*;
use crate::adapter::AdAdapter;
use crate::types::{AdFindSelectionKind, AdNativeHandle};
use agent_desktop_core::{ActionOps, InputOps, ObservationOps, SystemOps};
use agent_desktop_core::{
    LocatorField, ObservationRequest, ObservationRoot, ObservationSource, ObservedTree,
};

struct SwappedButtons;

impl ActionOps for SwappedButtons {}
impl InputOps for SwappedButtons {}
impl SystemOps for SwappedButtons {}

impl ObservationOps for SwappedButtons {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, agent_desktop_core::AdapterError> {
        match root {
            ObservationRoot::Window(window) => ObservedTree::from_roots(
                vec![named_subtree_at(
                    "Save",
                    true,
                    LocatorField::Known(agent_desktop_core::Rect {
                        x: 10.0,
                        y: 0.0,
                        width: 30.0,
                        height: 10.0,
                    }),
                )],
                ObservationSource::Window {
                    window: window.clone(),
                    surface: agent_desktop_core::SnapshotSurface::Window,
                },
                Default::default(),
                true,
            ),
            ObservationRoot::Element { .. } => element_tree("Delete", 10.0),
        }
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &agent_desktop_core::RefEntry,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::NativeHandle, agent_desktop_core::AdapterError> {
        Ok(agent_desktop_core::NativeHandle::new(10.0_f64))
    }
}

#[test]
fn first_selection_never_publishes_a_handle_that_landed_on_another_element() {
    let handle = crate::adapter::register_adapter(AdAdapter {
        inner: Box::new(SwappedButtons),
        session_id: None,
        _session_lease: None,
    })
    .unwrap();
    let mut find = query();
    find.control.selection.kind = AdFindSelectionKind::First as i32;
    let mut out = AdNativeHandle {
        ptr: std::ptr::null(),
    };
    let result = unsafe { ad_find_exact(handle, &ffi_window(), &find, &mut out) };
    unsafe { crate::adapter::ad_adapter_destroy(handle) };
    assert_eq!(result, AdResult::ErrStaleRef);
    assert!(out.ptr.is_null());
}

struct TwinButtons(&'static str);

impl ActionOps for TwinButtons {}
impl InputOps for TwinButtons {}
impl SystemOps for TwinButtons {}

impl ObservationOps for TwinButtons {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, agent_desktop_core::AdapterError> {
        let window = match root {
            ObservationRoot::Window(window) => window,
            ObservationRoot::Element { handle, .. } => {
                let x = *handle.downcast_ref::<f64>().unwrap_or(&-1.0);
                return element_tree(self.0, x);
            }
        };
        let roots = [10.0, 200.0]
            .into_iter()
            .map(|x| {
                subtree_at(
                    true,
                    LocatorField::Known(agent_desktop_core::Rect {
                        x,
                        y: 0.0,
                        width: 30.0,
                        height: 10.0,
                    }),
                )
            })
            .collect();
        ObservedTree::from_roots(
            roots,
            ObservationSource::Window {
                window: window.clone(),
                surface: agent_desktop_core::SnapshotSurface::Window,
            },
            Default::default(),
            true,
        )
    }

    fn resolve_element_strict(
        &self,
        _entry: &agent_desktop_core::RefEntry,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::NativeHandle, agent_desktop_core::AdapterError> {
        Err(agent_desktop_core::AdapterError::ambiguous_target(
            "identity is not unique",
        ))
    }

    fn resolve_locator_anchor(
        &self,
        entry: &agent_desktop_core::RefEntry,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::NativeHandle, agent_desktop_core::AdapterError> {
        let x = entry.geometry.bounds.map_or(-1.0, |bounds| bounds.x);
        Ok(agent_desktop_core::NativeHandle::new(x))
    }
}

fn find_twin(kind: AdFindSelectionKind, nth: u32) -> (AdResult, Option<f64>) {
    find_twin_landing(kind, nth, "Save")
}

fn find_twin_landing(
    kind: AdFindSelectionKind,
    nth: u32,
    landed: &'static str,
) -> (AdResult, Option<f64>) {
    let handle = crate::adapter::register_adapter(AdAdapter {
        inner: Box::new(TwinButtons(landed)),
        session_id: None,
        _session_lease: None,
    })
    .unwrap();
    let mut find = query();
    find.control.selection.kind = kind as i32;
    find.control.selection.nth = nth;
    let mut out = AdNativeHandle {
        ptr: std::ptr::null(),
    };
    let result = unsafe { ad_find_exact(handle, &ffi_window(), &find, &mut out) };
    let x = (result == AdResult::Ok).then(|| {
        let (native, _) =
            crate::actions::native_handle::acquire_ffi_handle(handle.addr(), &out).unwrap();
        *native.downcast_ref::<f64>().unwrap()
    });
    unsafe { crate::adapter::ad_adapter_destroy(handle) };
    (result, x)
}

#[test]
fn explicit_selection_among_identical_buttons_returns_the_chosen_element() {
    assert_eq!(
        find_twin(AdFindSelectionKind::Nth, 1),
        (AdResult::Ok, Some(200.0))
    );
    assert_eq!(
        find_twin(AdFindSelectionKind::Nth, 0),
        (AdResult::Ok, Some(10.0))
    );
    assert_eq!(
        find_twin(AdFindSelectionKind::First, 0),
        (AdResult::Ok, Some(10.0))
    );
    assert_eq!(
        find_twin(AdFindSelectionKind::Last, 0),
        (AdResult::Ok, Some(200.0))
    );
    assert_eq!(
        find_twin(AdFindSelectionKind::Strict, 0),
        (AdResult::ErrAmbiguousTarget, None)
    );
    assert_eq!(
        find_twin_landing(AdFindSelectionKind::Nth, 1, "Delete"),
        (AdResult::ErrStaleRef, None)
    );
}

fn element_tree(name: &str, x: f64) -> Result<ObservedTree, agent_desktop_core::AdapterError> {
    let bounds = LocatorField::Known(agent_desktop_core::Rect {
        x,
        y: 0.0,
        width: 30.0,
        height: 10.0,
    });
    ObservedTree::from_roots(
        vec![named_subtree_at(name, true, bounds)],
        ObservationSource::Window {
            window: crate::windows::ad_exact_window_to_core(&ffi_window())?,
            surface: agent_desktop_core::SnapshotSurface::Window,
        },
        Default::default(),
        true,
    )
}
