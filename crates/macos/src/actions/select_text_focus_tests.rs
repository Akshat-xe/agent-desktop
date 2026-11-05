use std::cell::Cell;

use accessibility_sys::AXUIElementCreateSystemWide;
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

use super::{FocusSnapshot, after_uncertain_write};
use crate::tree::AXElement;

fn focus_snapshot(element: AXElement) -> FocusSnapshot {
    FocusSnapshot {
        frontmost_pid: 406,
        frontmost_launch_time: Some(1.0),
        application: element.clone(),
        window: element.clone(),
        focused: element,
    }
}

fn uncertain_setter_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Selection setter outcome is uncertain",
    )
    .with_details(serde_json::json!({"ax_error": -25204}))
    .with_platform_detail("AXUIElementSetAttributeValue returned -25204")
    .with_disposition(DeliverySemantics::uncertain())
}

#[test]
fn uncertain_setter_error_records_unknown_focus_when_reread_fails() {
    let before = focus_snapshot(AXElement(unsafe { AXUIElementCreateSystemWide() }));
    let error = after_uncertain_write(uncertain_setter_error(), &before, || {
        Err(AdapterError::timeout("focus read failed"))
    });

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.message, "Selection setter outcome is uncertain");
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("AXUIElementSetAttributeValue returned -25204")
    );
    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    let details = error.details.unwrap();
    assert_eq!(details["ax_error"], -25204);
    assert_eq!(details["verification_scope"], "text_selection");
    assert!(details["focus_changed"].is_null());
    assert!(details["activated"].is_null());
    assert_eq!(details["focus_observation"], "unknown");
}

#[test]
fn uncertain_setter_error_records_observed_focus_when_reread_succeeds() {
    let element = AXElement(unsafe { AXUIElementCreateSystemWide() });
    let before = focus_snapshot(element.clone());
    let after = focus_snapshot(element);
    let error = after_uncertain_write(uncertain_setter_error(), &before, || Ok(after));

    assert_eq!(error.disposition, DeliverySemantics::uncertain());
    let details = error.details.unwrap();
    assert_eq!(details["focus_changed"], false);
    assert_eq!(details["activated"], false);
    assert_eq!(details["focus_observation"], "complete");
}

#[test]
fn not_delivered_setter_error_keeps_prewrite_disposition_without_focus_reread() {
    let before = focus_snapshot(AXElement(unsafe { AXUIElementCreateSystemWide() }));
    let read_called = Cell::new(false);
    let original = AdapterError::new(ErrorCode::ActionNotSupported, "Selection is unsupported")
        .with_disposition(DeliverySemantics::not_delivered());
    let error = after_uncertain_write(original, &before, || {
        read_called.set(true);
        Err(AdapterError::timeout("focus read failed"))
    });

    assert!(!read_called.get());
    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert_eq!(error.message, "Selection is unsupported");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(error.details.is_none());
}
