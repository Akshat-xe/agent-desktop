use serde_json::json;

use crate::{
    AdapterError, AppError, Deadline, ErrorCode, IdentityMatch, PlatformAdapter, RefEntry,
    WindowInfo,
    adapter::{NativeHandle, TreeOptions},
    locator::live_locator::{
        ObservationRequest, ObservationRoot, ObservedNode, ObservedTree, materialize_ref_entry,
    },
    refs::ref_identity::{PredicateEvidence, element_token, full_predicate_match},
    refs::resolve_errors::identity_unknown_error,
};

/// The depth cap of the search a saved ref gets: the same raw depth a snapshot
/// walk uses, so a ref taken from the deepest snapshot node is still inside the
/// scope it is resolved against.
const SCOPE_MAX_DEPTH: u8 = 50;

/// Resolves a saved ref under the reviewed identity rule.
///
/// A genuine retained element token resolves through the adapter, which owns
/// re-identification on that platform. Every other entry resolves only when it
/// was unique at capture and exactly one live element still matches its full
/// recorded predicate over complete evidence of the window it was captured
/// from; anything less is refused instead of guessed at.
pub fn resolve_saved_ref(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    if element_token(entry).is_some() {
        return adapter.resolve_element_strict(entry, deadline);
    }
    if !entry.identity.unique_at_capture {
        return Err(identity_not_unique(entry));
    }
    let window = recorded_window(adapter, entry, deadline)?;
    let observed = observe_scope(adapter, entry, &window, deadline)?;
    let found = unique_live_match(entry, &observed)?;
    let anchor = positional_anchor(entry, &observed, found)?;
    adapter.resolve_locator_anchor(&anchor, deadline)
}

/// The window the ref was captured from, never any other window of the
/// process: resolution re-searches the scope the capture proved unique.
///
/// The surface lookup owns shell surfaces and the focused-window case; a
/// recorded window that the inventory lists without surfacing through that
/// lookup is found by its id and process generation in the same inventory.
fn recorded_window(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    let resolved = crate::snapshot::resolve_window_for_surface(
        adapter,
        entry.source.source_app.as_deref(),
        entry.source.source_window_id.as_deref(),
        entry.source.source_surface,
        deadline,
    );
    let mut failure = None;
    match resolved {
        Ok(window) if belongs_to_entry(&window, entry) => return Ok(window),
        Ok(_) => {}
        Err(error) => failure = Some(adapter_error(error)),
    }
    let listed = adapter.list_windows(
        &crate::adapter::WindowFilter {
            focused_only: false,
            app: entry.source.source_app.clone(),
        },
        deadline,
    )?;
    listed
        .into_iter()
        .find(|window| belongs_to_entry(window, entry))
        .ok_or_else(|| {
            failure.unwrap_or_else(|| {
                AdapterError::stale_ref_because("The window the ref was captured from is gone")
            })
        })
}

fn belongs_to_entry(window: &WindowInfo, entry: &RefEntry) -> bool {
    window.pid == entry.process.pid
        && entry
            .source
            .source_window_id
            .as_deref()
            .is_none_or(|id| window.id == id)
        && window
            .process_instance
            .as_deref()
            .zip(entry.process.process_instance.as_deref())
            .is_none_or(|(live, saved)| live == saved)
}

fn observe_scope(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    window: &WindowInfo,
    deadline: Deadline,
) -> Result<ObservedTree, AdapterError> {
    let options = TreeOptions {
        max_depth: SCOPE_MAX_DEPTH,
        surface: entry.source.source_surface,
        ..TreeOptions::default()
    }
    .with_ref_identity_bounds();
    let request = ObservationRequest::snapshot(&options, deadline).validate()?;
    crate::tree::renderer_accessibility::observe_tree(
        adapter,
        ObservationRoot::Window(window),
        &request,
    )
    .map_err(adapter_error)
}

fn unique_live_match(entry: &RefEntry, observed: &ObservedTree) -> Result<usize, AdapterError> {
    let mut found = None;
    let mut incomplete = !observed.is_complete() || observed.nodes.iter().any(omits_descendants);
    for (index, node) in observed.nodes.iter().enumerate() {
        match full_predicate_match(entry, PredicateEvidence::observed(&node.evidence)) {
            IdentityMatch::Match if found.is_some() => return Err(ambiguous_live_match(entry)),
            IdentityMatch::Match => found = Some(index),
            IdentityMatch::Unknown => incomplete = true,
            IdentityMatch::NoMatch => {}
        }
    }
    if incomplete {
        return Err(identity_unknown_error(entry));
    }
    found.ok_or_else(|| absent(entry))
}

/// A container whose descendants were not read cannot support a uniqueness
/// verdict: the twin it might hide is exactly what the count is looking for.
fn omits_descendants(node: &ObservedNode) -> bool {
    !node.completeness.subtree_complete
        || (node.children_count.is_some_and(|count| count > 0) && node.children.is_empty())
}

/// A positional anchor at the matching node's path, carrying the saved
/// predicate: the adapter lands that path and its own anchor verification
/// re-checks the recorded identity, so a row that moved under the path is
/// refused rather than acted on.
fn positional_anchor(
    entry: &RefEntry,
    observed: &ObservedTree,
    index: usize,
) -> Result<RefEntry, AdapterError> {
    let node = observed
        .nodes
        .get(index)
        .ok_or_else(|| AdapterError::internal("saved ref match is missing from the observation"))?;
    let mut anchor = materialize_ref_entry(
        node,
        &observed.source,
        &crate::locator::LocatorQuery::default(),
    );
    anchor.identity = crate::RefEntryIdentity {
        retained_object: None,
        unique_at_capture: false,
        ..entry.identity.clone()
    };
    Ok(anchor)
}

fn identity_not_unique(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::AmbiguousTarget,
        "Saved ref was not unique when it was captured, so it cannot be re-identified safely",
    )
    .with_suggestion(
        "Take a new snapshot. Identical twins can only be picked by an explicit --headed \
         coordinate click, which takes the foreground.",
    )
    .with_details(json!({
        "kind": "identity_not_unique",
        "retryable": false,
        "role": entry.identity.role,
    }))
    .with_disposition(crate::DeliverySemantics::not_delivered())
}

fn ambiguous_live_match(entry: &RefEntry) -> AdapterError {
    AdapterError::ambiguous_target(
        "Two or more live elements match the saved ref's recorded role and stable identity",
    )
    .with_details(json!({
        "kind": "saved_ref_ambiguous",
        "retryable": false,
        "complete": true,
        "role": entry.identity.role,
    }))
}

fn absent(entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because("No live element matches the saved ref's recorded identity")
        .with_details(json!({
            "kind": "saved_ref_absent",
            "retryable": false,
            "complete": true,
            "role": entry.identity.role,
        }))
}

fn adapter_error(error: AppError) -> AdapterError {
    match error {
        AppError::Adapter(error) => error,
        other => AdapterError::internal(other.to_string()),
    }
}

#[cfg(test)]
#[path = "resolve_saved_ref_tests.rs"]
mod tests;
