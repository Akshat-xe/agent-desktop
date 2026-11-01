use super::{
    RetainedFacts, RetainedMismatch, native_identifier_attribute, resolve_saved_token,
    stale_token_error, token_mismatch,
};
use agent_desktop_core::{
    ElementIdentifier, ErrorCode, IdentifierKind, ProcessId, RefCapabilities, RefEntry,
    RefEntryIdentity, RefGeometry, RefProcess, RefScope, RefSource, SnapshotSurface,
};

fn entry(role: &str) -> RefEntry {
    RefEntry {
        process: RefProcess {
            pid: ProcessId::new(7),
            process_instance: Some("test-instance".into()),
        },
        identity: RefEntryIdentity {
            unique_at_capture: false,
            retained_object: None,
            role: role.into(),
            name: Some("Investors".into()),
            value: None,
            description: None,
            native_id: None,
        },
        geometry: RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: RefCapabilities {
            states: vec![],
            available_actions: vec![],
        },
        source: RefSource {
            source_app: None,
            source_window_id: Some("w-10".into()),
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: smallvec::smallvec![0, 1],
        },
    }
}

fn facts<'a>(
    pid: i32,
    role: Option<&'a str>,
    subrole: Option<&'a str>,
    native_id: Option<&'a str>,
) -> RetainedFacts<'a> {
    RetainedFacts {
        pid,
        role,
        subrole,
        native_id,
    }
}

#[test]
fn live_element_that_still_matches_verifies() {
    let stored = entry("button");

    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXButton"), None, None)),
        None
    );
}

#[test]
fn subrole_is_mapped_the_way_the_snapshot_mapped_it() {
    let treeitem = entry("treeitem");
    let row = entry("row");
    let outline_row = facts(7, Some("AXRow"), Some("AXOutlineRow"), None);

    assert_eq!(token_mismatch(&treeitem, &outline_row), None);
    assert_eq!(
        token_mismatch(&row, &outline_row),
        Some(RetainedMismatch::Role)
    );
}

#[test]
fn an_element_from_another_process_is_never_accepted() {
    let stored = entry("button");

    assert_eq!(
        token_mismatch(&stored, &facts(8, Some("AXButton"), None, None)),
        Some(RetainedMismatch::Pid)
    );
}

#[test]
fn a_different_role_is_never_accepted() {
    let stored = entry("button");

    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXCell"), None, None)),
        Some(RetainedMismatch::Role)
    );
    assert_eq!(
        token_mismatch(&stored, &facts(7, None, None, None)),
        Some(RetainedMismatch::Role)
    );
    assert_eq!(
        token_mismatch(
            &stored,
            &facts(7, Some("AXRoleThatDoesNotExist"), None, None)
        ),
        Some(RetainedMismatch::Role)
    );
}

#[test]
fn a_recorded_native_identifier_must_still_match() {
    let mut stored = entry("button");
    stored.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "submit-btn".into(),
    });

    assert_eq!(
        token_mismatch(
            &stored,
            &facts(7, Some("AXButton"), None, Some("submit-btn"))
        ),
        None
    );
    assert_eq!(
        token_mismatch(
            &stored,
            &facts(7, Some("AXButton"), None, Some("other-btn"))
        ),
        Some(RetainedMismatch::NativeId)
    );
    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXButton"), None, None)),
        Some(RetainedMismatch::NativeId)
    );
}

#[test]
fn a_recorded_dom_identifier_is_compared_as_a_dom_identifier() {
    let mut stored = entry("button");
    stored.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxDomIdentifier,
        value: "compose".into(),
    });

    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXButton"), None, Some("compose"))),
        None
    );
    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXButton"), None, Some("other"))),
        Some(RetainedMismatch::NativeId)
    );
    assert_eq!(
        native_identifier_attribute(IdentifierKind::AxDomIdentifier),
        Some("AXDOMIdentifier")
    );
}

#[test]
fn an_identifier_kind_macos_cannot_read_back_is_not_used() {
    let mut stored = entry("button");
    stored.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AutomationId,
        value: "save-button".into(),
    });

    assert_eq!(
        native_identifier_attribute(IdentifierKind::AutomationId),
        None
    );
    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXButton"), None, None)),
        None
    );
}

#[test]
fn a_blank_recorded_native_identifier_is_not_checked() {
    let mut stored = entry("button");
    stored.identity.native_id = Some(ElementIdentifier {
        kind: IdentifierKind::AxIdentifier,
        value: "   ".into(),
    });

    assert_eq!(
        token_mismatch(&stored, &facts(7, Some("AXButton"), None, None)),
        None
    );
}

#[test]
fn refs_without_a_token_keep_the_attribute_path() {
    let stored = entry("button");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

    assert!(resolve_saved_token(&stored, deadline).unwrap().is_none());
}

#[test]
fn a_stale_token_reports_the_refresh_suggestion() {
    let stored = entry("button");
    let error = stale_token_error(&stored, RetainedMismatch::Dead);

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Run 'snapshot' to refresh, then retry with the updated ref.")
    );
    let details = error.details.expect("stale token details");
    assert_eq!(details["kind"], "retained_object_stale");
    assert_eq!(details["reason"], "dead");
}
