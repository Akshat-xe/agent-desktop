use crate::{
    AppError, MouseButton, MouseEvent, MouseEventKind,
    adapter::PlatformAdapter,
    commands::{
        helpers::{apply_post_action_wait, validate_post_action_wait},
        operation_response::operation_response,
        point_resolve::{
            PointResolveArgs, ResolvedPoint, require_cursor_policy, require_pointer_capability,
        },
        pointer_action::{
            PointResolveAttempt, before_first_event, ensure_point_deadline,
            focus_point_under_lease, point_deadline, resolve_point_under_lease,
            retry_leased_point_phase, wait_for_point_with_deadline,
        },
    },
    context::CommandContext,
};
use serde_json::{Value, json};

pub struct HoverArgs {
    pub ref_id: Option<String>,
    pub xy: Option<(f64, f64)>,
    pub duration_ms: Option<u64>,
    pub timeout_ms: Option<u64>,
}

pub fn execute(
    args: HoverArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    if args.duration_ms.is_some_and(|duration| duration > 0) {
        return Err(AppError::invalid_input_with_suggestion(
            "Hover duration is unavailable because a stateless process cannot guarantee cursor ownership during a dwell",
            "Run hover without --duration, then use `wait <ms>` for an explicit post-hover pause.",
        ));
    }
    require_cursor_policy(context, "hover")?;
    require_pointer_capability(adapter)?;
    validate_post_action_wait(context)?;
    let deadline = point_deadline(args.timeout_ms)?;
    let point_args = PointResolveArgs {
        ref_id: args.ref_id.as_deref(),
        xy: args.xy,
        missing_input_message: "Provide a ref (@<snapshot_id>:e1) or --xy x,y",
        headed_requirement: crate::HeadedRequirement::FocusedWindowAndCursor,
    };
    let auto_wait = args.timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    let cursor = crate::cursor_overlay::hold_cursor(context, deadline);
    let (resolved, lease) = before_first_event(|| {
        if auto_wait {
            wait_for_point_with_deadline(point_args, deadline, adapter, context)?;
        }
        let (resolved, lease) = retry_leased_point_phase(args.timeout_ms, deadline, || {
            let lease = adapter.acquire_interaction_lease(deadline)?;
            let focused = focus_point_under_lease(point_args, &lease, adapter, context)?;
            let first = resolve_point_under_lease(
                PointResolveAttempt {
                    args: point_args,
                    stability: None,
                    allow_scroll: !auto_wait,
                },
                deadline,
                &lease,
                adapter,
                context,
            )?;
            let mut resolved = resolve_point_under_lease(
                PointResolveAttempt {
                    args: point_args,
                    stability: Some(first.bounds_hash),
                    allow_scroll: false,
                },
                deadline,
                &lease,
                adapter,
                context,
            )?;
            resolved.focused = focused;
            Ok((resolved, lease))
        })?;
        confirm_receiver(
            &resolved,
            deadline,
            adapter,
            args.ref_id.as_deref(),
            context,
        )?;
        ensure_point_deadline(
            deadline,
            Some(json!({
                "status": "actionable",
                "point": { "x": resolved.point.x, "y": resolved.point.y }
            })),
        )?;
        Ok((resolved, lease))
    })?;
    let result = crate::cursor_overlay::dispatch_mouse_event_with_cursor(
        adapter,
        context,
        MouseEvent {
            kind: MouseEventKind::Move,
            point: resolved.point.clone(),
            button: MouseButton::Left,
            modifiers: Vec::new(),
        },
        false,
        &lease,
    );
    let mut response = operation_response(
        &result?,
        json!({ "x": resolved.point.x, "y": resolved.point.y }),
    )?;
    if resolved.focused {
        response["focused"] = json!(true);
    }
    drop(lease);
    drop(cursor);
    apply_post_action_wait(response, resolved.source_entry.as_ref(), adapter, context)
}

fn confirm_receiver(
    resolved: &ResolvedPoint,
    deadline: crate::Deadline,
    adapter: &dyn PlatformAdapter,
    ref_id: Option<&str>,
    context: &CommandContext,
) -> Result<(), AppError> {
    let Some(entry) = resolved.source_entry.as_ref() else {
        return Ok(());
    };
    if entry.scope.root_ref.is_some() {
        let ref_id = ref_id.ok_or_else(|| {
            crate::AdapterError::stale_ref_because("Scoped pointer target has no source ref")
        })?;
        let handle = crate::refs::ref_resolve::resolve(adapter, entry, ref_id, context, deadline)?;
        if !matches!(
            adapter.hit_test(&handle, resolved.point.clone(), deadline)?,
            crate::adapter::hit_test::HitTestResult::ReachesTarget
        ) {
            return Err(crate::AdapterError::new(
                crate::ErrorCode::ActionFailed,
                "The pointer receiver inside the ref's source root could not be confirmed",
            )
            .with_details(json!({ "check": "receives_events" }))
            .with_disposition(crate::DeliverySemantics::not_delivered())
            .into());
        }
    } else {
        adapter.confirm_pointer_receiver(entry, resolved.point.clone(), deadline)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "hover_scope_tests.rs"]
mod scope_tests;
