use std::collections::HashSet;

use crate::{
    AccessibilityNode, AppError, WindowInfo,
    adapter::{PlatformAdapter, TreeOptions, WindowFilter},
    context::CommandContext,
    locator::live_locator::{ObservationRequest, ObservationRoot},
    refs::ref_alloc::{self, RefAllocConfig},
    refs::ref_alloc_options::RefAllocOptions,
    refs::ref_alloc_scope::RefAllocScope,
    refs::ref_alloc_source::RefAllocSource,
    refs::refs_store::RefStore,
    snapshot::SnapshotResult,
};

/// What one drill-down presents: the window it reports, the subtree that
/// replaces the stored refs, the path coordinates its refs are recorded in, and
/// the capture census of the observation that subtree came from. A token
/// re-identifies itself, so its observation is the subtree alone and carries no
/// census.
struct DrillCapture {
    window: WindowInfo,
    tree: AccessibilityNode,
    nodes_observed: usize,
    path_prefix: crate::refs::RefPath,
    unique: Option<HashSet<Vec<usize>>>,
}

#[cfg(test)]
pub fn run_from_ref(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    root_ref_id: &str,
    snapshot_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    run_from_ref_with_context(
        adapter,
        opts,
        &crate::refs::ref_token::qualify_for_test(root_ref_id, snapshot_id),
        &CommandContext::default(),
        crate::snapshot::DEFAULT_SNAPSHOT_TIMEOUT_MS,
    )
}

/// Drills into the stored ref `root_ref_id` (snapshot-qualified), replacing that subtree's refs in the
/// existing snapshot - `timeout_ms` is the observation deadline (A16-11),
/// threaded into both the drill-down read and the strict re-resolution that
/// precedes it.
///
/// A drill-down replaces refs inside an existing snapshot, so it must observe
/// its whole subtree or leave the stored map untouched. Partial projection is
/// only safe for a full snapshot, which writes a fresh map and destroys nothing.
///
/// A root without an element token is re-identified over a complete observation
/// of its window, so the subtree it drills is projected from that same
/// observation: the refs it writes were counted against the window they will be
/// resolved in, and the walk that proved the root doubles as their evidence.
pub fn run_from_ref_with_context(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    root_ref_id: &str,
    context: &CommandContext,
    timeout_ms: u64,
) -> Result<SnapshotResult, AppError> {
    let store = RefStore::for_session(context.session_id())?;
    let (active_snapshot_id, local_root_ref) =
        crate::refs::ref_token::resolve_ref_target(root_ref_id)?;
    let refmap = store.load_snapshot(&active_snapshot_id)?;

    let entry = refmap
        .get(&local_root_ref)
        .ok_or_else(|| AppError::stale_ref(root_ref_id))?
        .clone();

    let deadline = crate::Deadline::after(timeout_ms)?;
    let capture = drill_capture(adapter, opts, &entry, &local_root_ref, deadline)?;
    let DrillCapture {
        window,
        tree: raw_tree,
        nodes_observed,
        path_prefix,
        unique,
    } = capture;

    let source_app = entry.source.source_app.as_deref();
    let source_window_id = entry.source.source_window_id.as_deref();
    let source_window_title = entry.source.source_window_title.as_deref();
    let config = RefAllocConfig {
        options: RefAllocOptions {
            include_bounds: opts.include_bounds,
            interactive_only: opts.interactive_only,
            compact: opts.compact,
        },
        source: RefAllocSource {
            pid: entry.process.pid,
            app: source_app,
            window_id: source_window_id,
            window_title: source_window_title,
            window_bounds_hash: entry.source.source_window_bounds_hash,
            process_instance: entry.process.process_instance.as_deref(),
            surface: entry.source.source_surface,
        },
        scope: RefAllocScope {
            root_ref_id: Some(&local_root_ref),
            path_prefix: path_prefix.as_slice(),
        },
        unique_entries: unique.as_ref(),
    };

    let (mut tree, refmap) = store.update_existing_snapshot(
        &active_snapshot_id,
        &local_root_ref,
        &entry,
        |current| {
            current.remove_by_root_ref(&local_root_ref);
            let mut tree = ref_alloc::allocate_refs(raw_tree, current, &config)?;
            crate::snapshot::hints::add_structural_hints(&mut tree);
            Ok(tree)
        },
    )?;
    crate::trace::trace_artifacts::copy_refmap_if_full(
        context,
        &store,
        &active_snapshot_id,
        &refmap,
    )?;
    crate::refs::ref_token::qualify_tree_refs(&mut tree, &active_snapshot_id);
    context.trace_lazy("snapshot.root.saved", || {
        serde_json::json!({
            "root_ref": root_ref_id,
            "snapshot_id": active_snapshot_id,
            "ref_count": refmap.len()
        })
    })?;

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
        snapshot_id: Some(active_snapshot_id),
        complete: true,
        nodes_observed,
    })
}

fn drill_capture(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    entry: &crate::refs::RefEntry,
    local_root_ref: &str,
    deadline: crate::Deadline,
) -> Result<DrillCapture, AppError> {
    if crate::refs::ref_identity::element_token(entry).is_some() {
        return token_capture(adapter, opts, entry, local_root_ref, deadline);
    }
    let (window, observed, index) =
        crate::refs::resolve_saved_ref::observe_saved_ref(adapter, entry, deadline, None)?;
    let path_prefix = observed
        .nodes
        .get(index)
        .map(|node| node.path.clone())
        .ok_or_else(|| {
            AppError::Adapter(crate::AdapterError::internal(
                "saved ref match is missing from the observation",
            ))
        })?;
    let matched = u32::try_from(index).map_err(|_| {
        AppError::Adapter(crate::AdapterError::internal(
            "saved ref match is beyond the observation index space",
        ))
    })?;
    let unique =
        crate::refs::ref_alloc_unique::unique_capture_paths_under(&observed, matched, &path_prefix);
    let nodes_observed = observed.nodes.len();
    let tree = observed
        .into_accessibility_subtree(index, ObservationRequest::snapshot_logical_depth(opts))?;
    Ok(DrillCapture {
        window,
        tree,
        nodes_observed,
        path_prefix,
        unique: Some(unique),
    })
}

fn token_capture(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    entry: &crate::refs::RefEntry,
    local_root_ref: &str,
    deadline: crate::Deadline,
) -> Result<DrillCapture, AppError> {
    let window = resolve_result_window(entry, adapter, deadline)?;
    let handle = adapter.resolve_element_strict(entry, deadline)?;
    let observation_options = opts.with_ref_identity_bounds();
    let tree = crate::tree::renderer_accessibility::observe_tree(
        adapter,
        ObservationRoot::Element {
            handle: &handle,
            entry,
            root_ref: Some(local_root_ref),
        },
        &ObservationRequest::snapshot(&observation_options, deadline).validate()?,
        None,
    )?
    .into_accessibility_tree()?;
    let nodes_observed = count_nodes(&tree);
    Ok(DrillCapture {
        window,
        tree,
        nodes_observed,
        path_prefix: entry.scope.path.clone(),
        unique: None,
    })
}

/// The drill result reports the window the ref was captured from, so it must
/// resolve that exact window — not any window of the process. Resolving before
/// observation keeps a failed drill from persisting refs it never returns.
fn resolve_result_window(
    entry: &crate::refs::RefEntry,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<crate::WindowInfo, AppError> {
    let Some(window_id) = entry.source.source_window_id.as_deref() else {
        let instance = entry.process.process_instance.as_deref().ok_or_else(|| {
            AppError::Adapter(crate::AdapterError::stale_ref_because(
                "root ref has no process-instance identity",
            ))
        })?;
        return crate::system::window_lookup::find_window_for_process(
            crate::ProcessIdentity::new(entry.process.pid, instance),
            adapter,
            deadline,
        );
    };
    let candidates = adapter
        .list_windows(
            &WindowFilter {
                focused_only: false,
                app: entry.source.source_app.clone(),
            },
            deadline,
        )?
        .into_iter()
        .filter(|window| window.id == window_id && window.pid == entry.process.pid)
        .collect();
    crate::system::window_lookup::select_window(
        candidates,
        crate::AdapterError::new(
            crate::ErrorCode::WindowNotFound,
            format!("Source window '{window_id}' was not found"),
        )
        .with_suggestion("Run 'snapshot' to refresh, then retry with an updated ref"),
        "Multiple windows matched the source window ID",
    )
}

fn count_nodes(node: &AccessibilityNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

#[cfg(test)]
#[path = "snapshot_ref_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "snapshot_ref_alloc_tests.rs"]
mod alloc_tests;
