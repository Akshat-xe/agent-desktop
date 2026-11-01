use agent_desktop_core::{
    AdapterError, EvidenceRequirements, IdentityMatch, NativeHandle, RefEntry,
    refs::ref_identity::{has_stable_text_identity, identity_match},
};

use super::resolve_classify::classify_candidates;
use super::resolve_errors::identity_unknown as identity_unknown_error;
pub(in crate::tree) use super::resolve_errors::incomplete_traversal as incomplete_traversal_error;
use super::resolve_read_context::ResolveReadContext;
use crate::tree::AXElement;
use crate::tree::element_dedupe::push_unique;

#[cfg(target_os = "macos")]
pub(in crate::tree) fn find_entry_by_path(
    roots: &[AXElement],
    entry: &RefEntry,
    source_window_verified: bool,
    read_context: &mut ResolveReadContext,
) -> Result<NativeHandle, AdapterError> {
    crate::tree::locator_deadline::remaining(read_context.deadline)?;
    let mut matches = Vec::new();
    let mut incomplete = None;
    for root in roots {
        crate::tree::locator_deadline::remaining(read_context.deadline)?;
        let Some(candidate) = element_at_path(root, &entry.scope.path, read_context)? else {
            continue;
        };
        let identity = candidate_identity(&candidate, entry, read_context)?;
        match identity {
            IdentityMatch::Match => {
                push_unique(&mut matches, candidate);
            }
            IdentityMatch::Unknown => {
                incomplete.get_or_insert_with(|| identity_unknown_error(entry));
            }
            IdentityMatch::NoMatch => {}
        }
        if should_stop_collecting(matches.len(), entry) {
            break;
        }
    }
    if should_stop_collecting(matches.len(), entry) {
        return classify_candidates(
            matches,
            entry,
            source_window_verified,
            read_context.deadline,
        );
    }
    if let Some(error) = incomplete {
        return Err(error);
    }
    classify_candidates(
        matches,
        entry,
        source_window_verified,
        read_context.deadline,
    )
}

#[cfg(target_os = "macos")]
fn element_at_path(
    root: &AXElement,
    path: &[usize],
    context: &mut ResolveReadContext,
) -> Result<Option<AXElement>, AdapterError> {
    walk_finite_path(root.clone(), path, |current, index| {
        crate::tree::locator_deadline::remaining(context.deadline)?;
        if !context.usage.claim_node() {
            context.stats.traversal.limits.node_hits += 1;
            return Err(incomplete_traversal_error("path_node_budget", 0));
        }
        context.stats.reads.counts.attribute_batches += 1;
        context.stats.reads.counts.attributes_requested += 1;
        let role = super::resolve_ax_read::read_string(&current, "AXRole", context.deadline)?;
        let mut read = crate::tree::query::child_read::read_child_at(
            &current,
            role.as_deref(),
            index,
            context.deadline,
        );
        record_path_child_read(&read, context);
        crate::tree::locator_deadline::remaining(context.deadline)?;
        if read.status.api_disabled {
            return Err(AdapterError::permission_denied());
        }
        if !read.complete {
            return Err(incomplete_traversal_error("path_children", 0));
        }
        context
            .usage
            .claim_edges(usize::from(!read.elements.is_empty()));
        Ok(read.elements.pop())
    })
}

pub(in crate::tree) fn walk_finite_path<T>(
    mut current: T,
    path: &[usize],
    mut read_child: impl FnMut(T, usize) -> Result<Option<T>, AdapterError>,
) -> Result<Option<T>, AdapterError> {
    for index in path {
        let Some(child) = read_child(current, *index)? else {
            return Ok(None);
        };
        current = child;
    }
    Ok(Some(current))
}

#[cfg(target_os = "macos")]
fn record_path_child_read(
    read: &crate::tree::query::child_read::ChildRead,
    context: &mut ResolveReadContext,
) {
    context.stats.reads.counts.child_reads += read.status.attempts;
    context.stats.reads.health.cannot_complete += read.status.health.cannot_complete;
    context.stats.reads.health.native_read_failures += read.status.health.native_read_failures;
    context.stats.reads.health.deadline_exhausted += read.status.health.deadline_exhausted;
    context.stats.traversal.limits.child_count_changes += u64::from(read.status.count_changed);
    context.stats.traversal.limits.child_hits += u64::from(read.status.cursor_stalled);
}

#[cfg(target_os = "macos")]
fn candidate_identity(
    element: &AXElement,
    entry: &RefEntry,
    context: &mut ResolveReadContext,
) -> Result<IdentityMatch, AdapterError> {
    let read = read_node(element, identity_requirements(entry), context)?;
    if read.invalid_element {
        return Ok(IdentityMatch::Unknown);
    }
    let matched = match read.evidence.role.known() {
        Some(role) if role == &entry.identity.role => {
            let identity = super::retained::candidate_match(element, entry, &read.evidence)?;
            if identity == IdentityMatch::Unknown && provisional_geometry_candidate(entry) {
                IdentityMatch::Match
            } else {
                identity
            }
        }
        Some(_) => IdentityMatch::NoMatch,
        None if read.evidence.role.is_unknown() => IdentityMatch::Unknown,
        None => IdentityMatch::NoMatch,
    };
    Ok(matched)
}

pub(in crate::tree) fn match_native_or_text_identity(
    entry: &RefEntry,
    evidence: &agent_desktop_core::LocatorEvidence,
) -> IdentityMatch {
    identity_match(
        entry,
        &evidence.name,
        &evidence.value,
        &evidence.description,
        &evidence.identifiers,
    )
}

/// Whether bounds alone may stand in for an element whose identity is unknown.
/// A ref that recorded a token never qualifies: only that token may match it,
/// which `has_meaningful_identity` already enforces.
fn provisional_geometry_candidate(entry: &RefEntry) -> bool {
    entry.geometry.bounds_hash.is_some()
        && !agent_desktop_core::refs::ref_identity::has_meaningful_identity(entry)
}

#[cfg(target_os = "macos")]
fn read_node(
    element: &AXElement,
    requirements: EvidenceRequirements,
    context: &mut ResolveReadContext,
) -> Result<crate::tree::query::node_read::NodeRead, AdapterError> {
    if !context.usage.claim_node() {
        context.stats.traversal.limits.node_hits += 1;
        return Err(incomplete_traversal_error("node_budget", 0));
    }
    let child_plan =
        crate::tree::query::child_read_plan::ChildReadPlan::load(context.usage.child_capacity());
    let read = crate::tree::query::node_read::read_node(
        element,
        crate::tree::query::node_read_context::NodeReadContext {
            tree: &crate::tree::TreeBuildContext::empty(false),
            stats: &mut context.stats,
            usage: &mut context.usage,
            requirements,
            deadline: context.deadline,
            child_plan,
        },
    )?;
    context
        .usage
        .note_child_demand(read.child_read.total_count, &mut context.stats);
    context.usage.claim_edges(read.child_read.elements.len());
    Ok(read)
}

fn identity_requirements(entry: &RefEntry) -> EvidenceRequirements {
    let semantic_text = has_stable_text_identity(entry) || entry.identity.role == "cell";
    EvidenceRequirements {
        role: true,
        name: semantic_text,
        description: semantic_text,
        value: semantic_text,
        identifiers: entry.identity.native_id.is_some(),
        states: false,
        ref_evidence: Default::default(),
    }
}

pub(in crate::tree) fn should_stop_collecting(match_count: usize, entry: &RefEntry) -> bool {
    match_count > 1 && entry.geometry.bounds_hash.is_none()
}

#[cfg(all(test, target_os = "macos"))]
#[path = "resolve_search_tests.rs"]
mod tests;
