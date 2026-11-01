use super::*;
use crate::{
    AdapterError,
    actions::action::Action,
    actions::capability,
    adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps},
};
use std::sync::atomic::{AtomicU32, Ordering};

/// Fixture ref entry for a button with known bounds.
fn entry() -> RefEntry {
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Run"));
    entry.geometry.bounds = Some(bounds);
    entry.geometry.bounds_hash = bounds.bounds_hash();
    entry.capabilities.available_actions = vec![capability::CLICK.into()];
    entry.source.source_app = Some("Original".into());
    entry
}

struct SuccessWithUnresponsiveProbeAdapter {
    probe_calls: AtomicU32,
}

impl ObservationOps for SuccessWithUnresponsiveProbeAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!("button", "Run", [capability::CLICK]);
}

impl ActionOps for SuccessWithUnresponsiveProbeAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<crate::actions::action_result::ActionResult, AdapterError> {
        Ok(crate::actions::action_result::ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for SuccessWithUnresponsiveProbeAdapter {}

impl SystemOps for SuccessWithUnresponsiveProbeAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn process_state(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<crate::system::process_state::ProcessState, AdapterError> {
        self.probe_calls.fetch_add(1, Ordering::SeqCst);
        Ok(crate::system::process_state::ProcessState::Unresponsive)
    }
}

#[test]
fn enrichment_never_converts_a_successful_action_into_a_failure() {
    let adapter = SuccessWithUnresponsiveProbeAdapter {
        probe_calls: AtomicU32::new(0),
    };

    let result = execute_with_auto_wait(
        RefActionWaitCtx {
            adapter: &adapter,
            entry: &entry(),
            ref_id: "@e1",
            context: &CommandContext::default(),
        },
        ActionRequest::headless(Action::Click),
        crate::ref_action::dispatch_resolved,
    )
    .unwrap();
    assert_eq!(result.action, "click");
    assert_eq!(adapter.probe_calls.load(Ordering::SeqCst), 0);
}
