//! The Dock and the system menu bar extras: surfaces no window owns, found by
//! the process that presents them.

use super::{AXElement, properties::element::element_for_pid, surface_read};
use agent_desktop_core::{AdapterError, ErrorCode, SnapshotSurface, WindowInfo};
use std::time::Instant;

/// MenuBarAgent hosts the system status items on current macOS; Control Center
/// and SystemUIServer hosted them on earlier releases.
const MENU_BAR_EXTRAS_HOSTS: [&str; 3] = [
    "com.apple.MenuBarAgent",
    "com.apple.controlcenter",
    "com.apple.systemuiserver",
];

/// The surface's root in `pid`, or `None` when that process does not present it.
pub(crate) fn root_for(
    surface: SnapshotSurface,
    pid: i32,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let app = element_for_pid(pid);
    match surface {
        SnapshotSurface::MenuBarExtras => surface_read::element(&app, "AXExtrasMenuBar", deadline),
        SnapshotSurface::Dock => Ok(Some(app)),
        _ => Ok(None),
    }
}

pub(crate) fn resolve_surface(
    surface: SnapshotSurface,
    deadline: Instant,
) -> Result<WindowInfo, AdapterError> {
    let hosts: &[&str] = match surface {
        SnapshotSurface::Dock => &["com.apple.dock"],
        SnapshotSurface::MenuBarExtras => &MENU_BAR_EXTRAS_HOSTS,
        _ => return Err(AdapterError::not_supported("resolve_shell_surface")),
    };
    let apps = crate::system::app::workspace_apps::list_apps_inventory_until(deadline)?.apps;
    for host in hosts {
        for app in apps
            .iter()
            .filter(|app| app.bundle_id.as_deref() == Some(*host))
        {
            let pid = crate::system::process::process_identity::to_pid_t(app.pid)?;
            if root_for(surface, pid, deadline)?.is_some() {
                return Ok(WindowInfo::for_process(app));
            }
        }
    }
    Err(AdapterError::new(
        ErrorCode::WindowNotFound,
        format!(
            "No running process presents the '{}' surface",
            surface.as_str()
        ),
    ))
}
