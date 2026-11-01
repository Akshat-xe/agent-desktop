//! The identity-uniqueness tier: what a live count decides, which candidates it
//! counts, and the two cut regions a count must never resolve past.

use super::*;
use crate::tree::walk::walker_fake::FakeTree;

/// An empty count only settles when every region was read; otherwise `STALE_REF` would be a
/// verdict off a walk that never finished.
#[test]
fn an_empty_count_with_an_unread_region_is_incomplete_not_stale() {
    assert!(matches!(
        classify_identity_search(0, true),
        SearchVerdict::Incomplete
    ));
    assert!(matches!(
        classify_identity_search(0, false),
        SearchVerdict::Stale
    ));
}

/// A ref recorded before identity evidence was kept is refused from its own record, before any
/// window resolution or search.
#[test]
fn a_legacy_entry_is_refused_before_any_live_work() {
    let mut entry = crate::tree::walk::walker_fake::ref_entry("button");
    entry.source.source_window_id = Some("w-not-a-real-window".into());

    let error =
        crate::tree::resolve::resolve_attempt(&entry, crate::tree::walk::walker_fake::deadline())
            .err()
            .expect("a ref with no recorded basis must never resolve");

    assert_eq!(error.code, agent_desktop_core::ErrorCode::StaleRef);
    assert_eq!(
        error.message, "The saved ref was recorded before identity evidence was kept",
        "the refusal must name the missing basis, not a live mismatch"
    );
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

fn children_budget(max_siblings: usize) -> WalkBudget {
    crate::tree::walk::walker_fake::budget(10).with_max_siblings(max_siblings)
}

/// Access denied on a cap probe is the attempt's answer, not an unread region that retries turn
/// into a timeout.
#[test]
fn a_denied_boundary_probe_surfaces_perm_denied() {
    use crate::system::runtime::hresult::E_ACCESSDENIED;
    let denied = UiaFailure::Hresult(E_ACCESSDENIED);
    let deep = FakeTree::default().faulting_on_first_child_with(1, denied);
    let error = super::boundary_hides_descendants(&deep, &1).expect_err("terminal surfaces");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);

    let wide = FakeTree::default()
        .with_children(1, &[2, 3, 4])
        .faulting_on_next_sibling_with(3, denied);
    let error = super::sibling_list_was_cut(&wide, &[2, 3], &children_budget(2))
        .expect_err("terminal surfaces");
    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
}
