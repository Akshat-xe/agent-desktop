use agent_desktop_core::{AdapterError, ErrorCode, WindowInfo};
use rustc_hash::{FxHashMap, FxHashSet};
use std::time::Instant;

type AxWindowIdentity = (Option<String>, Option<i64>);

#[derive(Clone, Debug)]
pub(crate) struct WindowAxState {
    pub(crate) focused: Option<AxWindowIdentity>,
    pub(crate) minimized_by_id: FxHashMap<i64, bool>,
    pub(crate) accessible_window_ids: FxHashSet<i64>,
}

pub(crate) fn read_until(pid: i32, deadline: Instant) -> Result<WindowAxState, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    let focused = focused_identity(&app, deadline)?;
    let mut minimized_by_id = FxHashMap::default();
    let mut accessible_window_ids = FxHashSet::default();
    for_each_window(pid, deadline, |window, window_id| {
        accessible_window_ids.insert(window_id);
        if let Some(minimized) =
            crate::tree::surface_read::boolean(window, "AXMinimized", deadline)?
        {
            minimized_by_id.insert(window_id, minimized);
        }
        Ok(())
    })?;
    Ok(WindowAxState {
        focused,
        minimized_by_id,
        accessible_window_ids,
    })
}

pub(crate) fn read_frontmost_until(
    pid: i32,
    deadline: Instant,
) -> Result<WindowAxState, AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    Ok(WindowAxState {
        focused: focused_identity(&app, deadline)?,
        minimized_by_id: FxHashMap::default(),
        accessible_window_ids: FxHashSet::default(),
    })
}

pub(crate) fn accessible_window_ids_until(
    pid: i32,
    deadline: Instant,
) -> Result<FxHashSet<i64>, AdapterError> {
    let mut ids = FxHashSet::default();
    for_each_window(pid, deadline, |_, id| {
        ids.insert(id);
        Ok(())
    })?;
    Ok(ids)
}

fn for_each_window(
    pid: i32,
    deadline: Instant,
    mut visit: impl FnMut(&crate::tree::AXElement, i64) -> Result<(), AdapterError>,
) -> Result<(), AdapterError> {
    let app = crate::tree::element_for_pid(pid);
    for window in crate::tree::surface_read::elements(&app, "AXWindows", deadline)? {
        match crate::system::window::window_resolve::ax_window_id_with_deadline(&window, deadline) {
            Ok(Some(id)) => visit(&window, id)?,
            Ok(None) => {}
            Err(error) if crate::system::window::window_bridge::is_unavailable(&error) => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) fn label_accessibility(
    windows: &mut [WindowInfo],
    mut read: impl FnMut(i32) -> Result<FxHashSet<i64>, AdapterError>,
) {
    let mut pids = windows.iter().map(|window| window.pid).collect::<Vec<_>>();
    pids.sort_unstable();
    pids.dedup();
    for pid in pids {
        let Some(accessible_ids) = crate::system::process::process_identity::to_pid_t(pid)
            .ok()
            .and_then(|pid| read(pid).ok())
        else {
            continue;
        };
        for window in windows.iter_mut().filter(|window| window.pid == pid) {
            window.state.accessible =
                crate::system::window::window_resolve::parse_window_number(&window.id)
                    .is_some_and(|id| accessible_ids.contains(&id));
        }
    }
}

/// The AX window a focused accessibility object belongs to: the object itself when it is a
/// window, the window owning a sheet, dialog, alert or popover, and none otherwise.
pub(crate) fn owning_window(
    focused: &crate::tree::AXElement,
    deadline: Instant,
) -> Result<Option<crate::tree::AXElement>, AdapterError> {
    let role = crate::tree::surface_read::string(focused, "AXRole", deadline)?;
    match focused_role_kind(role.as_deref()) {
        FocusedRoleKind::Window => Ok(Some(focused.clone())),
        FocusedRoleKind::TransientSurface => {
            let Some(window) = crate::tree::surface_read::element(focused, "AXWindow", deadline)?
            else {
                return Ok(None);
            };
            if crate::tree::surface_read::string(&window, "AXRole", deadline)?.as_deref()
                != Some("AXWindow")
            {
                return Ok(None);
            }
            Ok(Some(window))
        }
        FocusedRoleKind::NotAWindow => {
            tracing::debug!(?role, "focused accessibility object is not a window");
            Ok(None)
        }
    }
}

/// Refuses a focused surface the recorded window does not own, since a snapshot pins every ref
/// it allocates to that one window.
pub(crate) fn verify_focused_window(
    window: &WindowInfo,
    focused: &crate::tree::AXElement,
    deadline: Instant,
) -> Result<(), AdapterError> {
    let number = match owning_window(focused, deadline)? {
        Some(owner) => {
            crate::system::window::window_resolve::ax_window_id_with_deadline(&owner, deadline)?
        }
        None => None,
    };
    if observes_recorded_window(&window.id, number) {
        return Ok(());
    }
    Err(crate::system::window::window_resolve::window_identity_mismatch(&window.id))
}

/// Whether a capture may record `recorded_id` for the observed surface: the window number
/// must match, and a number the bridge could not read leaves the window unproven.
fn observes_recorded_window(recorded_id: &str, observed_number: Option<i64>) -> bool {
    crate::system::window::window_resolve::parse_window_number(recorded_id)
        .zip(observed_number)
        .is_some_and(|(recorded, observed)| recorded == observed)
}

/// Frontmost-ness only selects whether a focused window is reported, and a busy
/// application answering "unknown" is not a reason to fail the command that
/// asked. Permission and API failures still propagate, because those describe
/// the caller's access rather than the application's state.
fn is_frontmost(app: &crate::tree::AXElement, deadline: Instant) -> Result<bool, AdapterError> {
    match crate::tree::surface_read::boolean(app, "AXFrontmost", deadline) {
        Ok(frontmost) => Ok(frontmost == Some(true)),
        Err(error) if error.code == ErrorCode::PermDenied => Err(error),
        Err(_) => Ok(false),
    }
}

fn focused_identity(
    app: &crate::tree::AXElement,
    deadline: Instant,
) -> Result<Option<AxWindowIdentity>, AdapterError> {
    if !is_frontmost(app, deadline)? {
        return Ok(None);
    }
    let Some(focused) = crate::tree::surface_read::element(app, "AXFocusedWindow", deadline)?
    else {
        return Ok(None);
    };
    let Some(window) = owning_window(&focused, deadline)? else {
        return Ok(None);
    };
    let title = crate::tree::surface_read::string(&window, "AXTitle", deadline)?;
    let number = match crate::system::window::window_resolve::ax_window_id_with_deadline(
        &window, deadline,
    ) {
        Ok(number) => number,
        Err(error) if crate::system::window::window_bridge::is_unavailable(&error) => None,
        Err(error) => return Err(error),
    };
    Ok(Some((title, number)))
}

fn is_transient_surface_role(role: &str) -> bool {
    matches!(role, "AXSheet" | "AXDialog" | "AXAlert" | "AXPopover")
}

#[derive(Debug, PartialEq)]
enum FocusedRoleKind {
    Window,
    TransientSurface,
    NotAWindow,
}

fn focused_role_kind(role: Option<&str>) -> FocusedRoleKind {
    match role {
        Some("AXWindow") => FocusedRoleKind::Window,
        Some(role) if is_transient_surface_role(role) => FocusedRoleKind::TransientSurface,
        _ => FocusedRoleKind::NotAWindow,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FocusedRoleKind, focused_role_kind, is_transient_surface_role, label_accessibility,
        observes_recorded_window,
    };
    use agent_desktop_core::{ProcessId, WindowInfo, WindowState};
    use rustc_hash::FxHashSet;

    #[test]
    fn a_focused_surface_in_another_window_is_refused_for_the_recorded_window() {
        assert!(
            !observes_recorded_window("w-11", Some(12)),
            "window w-12 holds focus, so capturing it as w-11 would pin every ref in \
             w-12's tree to a window those elements were never read from"
        );
    }

    #[test]
    fn the_focused_surface_of_the_recorded_window_is_captured() {
        assert!(observes_recorded_window("w-11", Some(11)));
    }

    #[test]
    fn an_unproven_focused_window_is_refused() {
        assert!(
            !observes_recorded_window("w-11", None),
            "a window number the bridge could not read cannot prove which window was observed"
        );
        assert!(!observes_recorded_window("", Some(11)));
        assert!(!observes_recorded_window("desktop", Some(11)));
    }

    #[test]
    fn modal_surfaces_can_temporarily_own_focused_window() {
        for role in ["AXSheet", "AXDialog", "AXAlert", "AXPopover"] {
            assert!(is_transient_surface_role(role));
        }
        assert!(!is_transient_surface_role("AXButton"));
    }

    #[test]
    fn a_focused_desktop_is_no_focused_window_rather_than_an_error() {
        assert_eq!(focused_role_kind(Some("AXWindow")), FocusedRoleKind::Window);
        assert_eq!(
            focused_role_kind(Some("AXSheet")),
            FocusedRoleKind::TransientSurface
        );
        assert_eq!(
            focused_role_kind(Some("AXScrollArea")),
            FocusedRoleKind::NotAWindow
        );
        assert_eq!(focused_role_kind(None), FocusedRoleKind::NotAWindow);
    }

    #[test]
    fn failed_pid_probe_keeps_unknown_window_selectable() {
        let mut windows = vec![window(10, "w-1"), window(20, "w-2")];

        label_accessibility(&mut windows, |pid| {
            if pid == 10 {
                Ok(FxHashSet::default())
            } else {
                Err(agent_desktop_core::AdapterError::timeout("busy"))
            }
        });

        assert!(!windows[0].state.accessible);
        assert!(windows[1].state.accessible);
    }

    fn window(pid: u32, id: &str) -> WindowInfo {
        WindowInfo {
            id: id.into(),
            title: "Main".into(),
            app: "Fixture".into(),
            pid: ProcessId::new(pid),
            process_instance: Some(format!("{pid}:100")),
            bounds: None,
            state: WindowState::default(),
        }
    }
}
