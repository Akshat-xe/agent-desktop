use super::*;
use agent_desktop_core::ErrorCode;
use std::time::{Duration, Instant};

fn focused_entry(window_id: Option<&str>, title: Option<&str>) -> RefEntry {
    let mut stored = crate::tree::resolve::tests::entry(Some(42), window_id, title, None);
    stored.identity.basis = Some(agent_desktop_core::IdentityBasis::Name);
    stored.source.source_surface = SnapshotSurface::Focused;
    stored
}

#[test]
fn a_focused_ref_is_pinned_to_the_window_it_was_captured_in() {
    let captured_in_window = focused_entry(Some("w-10"), Some("Editor"));

    assert_eq!(
        identity_root_scope(&captured_in_window),
        IdentityRootScope::RecordedWindow
    );
}

#[test]
fn a_focused_ref_with_only_a_recorded_title_is_still_pinned() {
    assert_eq!(
        identity_root_scope(&focused_entry(None, Some("Editor"))),
        IdentityRootScope::RecordedWindow
    );
}

#[test]
fn a_focused_ref_without_a_recorded_window_is_refused_instead_of_following_focus() {
    let unpinnable = focused_entry(None, Some(" "));
    assert_eq!(
        identity_root_scope(&unpinnable),
        IdentityRootScope::RefuseMissingWindow
    );

    let mut context = ResolveReadContext::new(Instant::now() + Duration::from_secs(1));
    let error = identity_scope_roots(&unpinnable, &mut context)
        .err()
        .expect("a focused ref that recorded no window must never search the live focus");

    assert_eq!(error.code, ErrorCode::ElementNotFound);
}

#[test]
fn a_focused_positional_ref_uses_the_same_recorded_window_scope() {
    let mut captured = focused_entry(Some("w-10"), Some("Editor"));
    captured.identity.basis = Some(agent_desktop_core::IdentityBasis::Positional);
    assert_eq!(
        identity_root_scope(&captured),
        IdentityRootScope::RecordedWindow
    );

    captured.source.source_window_id = None;
    captured.source.source_window_title = None;
    let mut context = ResolveReadContext::new(Instant::now() + Duration::from_secs(1));
    let error = candidate_roots(&captured, &mut context)
        .err()
        .expect("positional refs must not follow the live focused surface either");
    assert_eq!(error.code, ErrorCode::ElementNotFound);
    assert_eq!(
        error.message,
        AdapterError::element_not_found("saved source window").message
    );
}

#[test]
fn a_window_scoped_ref_keeps_the_existing_root_logic() {
    let mut window = crate::tree::resolve::tests::entry(Some(42), Some("w-42"), None, None);
    window.identity.basis = Some(agent_desktop_core::IdentityBasis::Name);
    assert_eq!(
        identity_root_scope(&window),
        IdentityRootScope::CapturedSurface
    );

    window.source.source_surface = SnapshotSurface::Menu;
    assert_eq!(
        identity_root_scope(&window),
        IdentityRootScope::CapturedSurface
    );
}
