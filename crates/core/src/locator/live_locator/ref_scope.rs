use super::{ObservationRequest, ObservationRoot, ObservedNode, ObservedTree};
use crate::{
    AdapterError, Deadline, IdentityBasis, IdentityMatch, NativeHandle, PlatformAdapter, RefEntry,
    refs::identity_basis_gate::requires_live_uniqueness,
    refs::ref_identity::{identity_match, retained_object_match},
};

/// A scope root together with the live handle and the anchor entry carrying its verified
/// current path, which can differ from the path recorded when the ref was captured.
pub(crate) struct ScopeRoot<'a> {
    pub(crate) captured: &'a RefEntry,
    pub(crate) current: RefEntry,
    pub(crate) handle: NativeHandle,
}

#[cfg(test)]
impl<'a> ScopeRoot<'a> {
    pub(crate) fn new(captured: &'a RefEntry, handle: NativeHandle) -> Self {
        Self {
            captured,
            current: captured.clone(),
            handle,
        }
    }
}

fn into_adapter(error: crate::AppError) -> AdapterError {
    match error {
        crate::AppError::Adapter(error) => error,
        error => AdapterError::internal(error.to_string()),
    }
}

pub(crate) fn resolve_in_root<'a>(
    adapter: &dyn PlatformAdapter,
    entry: &'a RefEntry,
    root: &ScopeRoot<'_>,
    deadline: Deadline,
) -> Result<ScopeRoot<'a>, AdapterError> {
    pin_root(adapter, &root.current, deadline)?;
    let observation_root = ObservationRoot::Element {
        handle: &root.handle,
        entry: &root.current,
        root_ref: entry.scope.root_ref.as_deref(),
    };
    let options = crate::TreeOptions {
        max_depth: 50,
        include_bounds: true,
        surface: root.current.source.source_surface,
        ..Default::default()
    };
    let tree = crate::tree::renderer_accessibility::observe_tree(
        adapter,
        observation_root,
        &ObservationRequest::snapshot(&options, deadline).validate()?,
    )
    .map_err(into_adapter)?;
    if requires_live_uniqueness(&entry.identity) && tree.omits_descendants() {
        return Err(incomplete());
    }
    let selected = select(&tree, entry, root.captured)?;
    let mut anchor =
        super::materialize::ref_entry(selected, &tree.source, &crate::LocatorQuery::default());
    anchor.identity = entry.identity.clone();
    anchor.geometry = entry.geometry.clone();
    let handle = resolve_anchor(adapter, &anchor, deadline)?;
    pin_root(adapter, &root.current, deadline)?;
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    Ok(ScopeRoot {
        captured: entry,
        current: anchor,
        handle,
    })
}

/// Resolves the first scope root and finds its current path, relocating it through its
/// window when its recorded path no longer holds it (a sibling was inserted before it).
pub(crate) fn resolve_first_root<'a>(
    adapter: &dyn PlatformAdapter,
    root: &'a RefEntry,
    deadline: Deadline,
) -> Result<ScopeRoot<'a>, AdapterError> {
    if let Some(depth) = root.scope.census_depth {
        let (handle, current) = locate_in_window(adapter, root, depth, deadline)?;
        return Ok(ScopeRoot {
            captured: root,
            current,
            handle,
        });
    }
    let handle = adapter.resolve_element_strict(root, deadline)?;
    let current = match resolve_anchor(adapter, root, deadline) {
        Ok(_) => root.clone(),
        Err(error) if error.code == crate::ErrorCode::PlatformNotSupported => root.clone(),
        Err(error) if error.code == crate::ErrorCode::StaleRef => {
            let depth = u8::try_from(root.scope.path.len())
                .map_err(|_| AdapterError::stale_ref_because("Ref path is too deep to relocate"))?;
            locate_in_window(adapter, root, depth, deadline)?.1
        }
        Err(error) => return Err(error),
    };
    Ok(ScopeRoot {
        captured: root,
        current,
        handle,
    })
}

/// Resolves an entry whose basis was proven over the first `depth` skeleton levels by observing
/// its window the same way and requiring exactly one match inside that depth.
pub(crate) fn resolve_by_census(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    depth: u8,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    locate_in_window(adapter, entry, depth, deadline).map(|(handle, _)| handle)
}

fn locate_in_window(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    depth: u8,
    deadline: Deadline,
) -> Result<(NativeHandle, RefEntry), AdapterError> {
    let window = crate::snapshot::snapshot_ref::resolve_result_window(entry, adapter, deadline)
        .map_err(into_adapter)?;
    let options = crate::TreeOptions {
        max_depth: depth,
        include_bounds: true,
        skeleton: true,
        surface: entry.source.source_surface,
        ..Default::default()
    };
    let tree = crate::tree::renderer_accessibility::observe_tree(
        adapter,
        ObservationRoot::Window(&window),
        &ObservationRequest::snapshot(&options, deadline).validate()?,
    )
    .map_err(into_adapter)?;
    let selected = select(&tree, entry, entry)?;
    let mut anchor =
        super::materialize::ref_entry(selected, &tree.source, &crate::LocatorQuery::default());
    anchor.identity = entry.identity.clone();
    anchor.geometry = entry.geometry.clone();
    anchor.scope.census_depth = entry.scope.census_depth;
    let handle = resolve_anchor(adapter, &anchor, deadline)?;
    Ok((handle, anchor))
}

pub(crate) fn pin_root(
    adapter: &dyn PlatformAdapter,
    current: &RefEntry,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    resolve_anchor(adapter, current, deadline).map(|_| ())
}

fn select<'a>(
    tree: &'a ObservedTree,
    entry: &RefEntry,
    root: &RefEntry,
) -> Result<&'a ObservedNode, AdapterError> {
    if !tree.is_complete() {
        return Err(incomplete());
    }
    if entry.identity.basis == Some(IdentityBasis::Positional) {
        let relative = entry
            .scope
            .path
            .strip_prefix(root.scope.path.as_slice())
            .ok_or_else(stale)?;
        let node = tree
            .nodes
            .iter()
            .find(|node| node.path.as_slice() == relative)
            .ok_or_else(stale)?;
        if !positional_matches(entry, node) {
            return Err(stale());
        }
        retained_object_verdict(entry, node)?;
        return Ok(node);
    }
    let mut selected = None;
    let mut unknown = false;
    for node in &tree.nodes {
        let Some(role) = node.evidence.role.known() else {
            unknown |= node.evidence.role.is_unknown();
            continue;
        };
        if role != &entry.identity.role {
            continue;
        }
        match identity_match(
            entry,
            &node.evidence.name,
            &node.evidence.value,
            &node.evidence.description,
            &node.evidence.identifiers,
        ) {
            IdentityMatch::Match if selected.is_some() => {
                return Err(AdapterError::ambiguous_target(
                    "Multiple elements match the ref inside its source root",
                ));
            }
            IdentityMatch::Match => selected = Some(node),
            IdentityMatch::Unknown => unknown = true,
            IdentityMatch::NoMatch => {}
        }
    }
    if unknown {
        return Err(incomplete());
    }
    let selected = selected.ok_or_else(stale)?;
    retained_object_verdict(entry, selected)?;
    Ok(selected)
}

fn retained_object_verdict(entry: &RefEntry, node: &ObservedNode) -> Result<(), AdapterError> {
    match retained_object_match(entry, &node.evidence.identifiers) {
        IdentityMatch::Match => Ok(()),
        IdentityMatch::NoMatch => Err(stale()),
        IdentityMatch::Unknown => Err(incomplete()),
    }
}

fn resolve_anchor(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    let mut anchor = entry.clone();
    anchor.identity.basis = Some(IdentityBasis::Positional);
    if requires_live_uniqueness(&entry.identity) {
        anchor.geometry.bounds = None;
        anchor.geometry.bounds_hash = None;
        anchor.identity.value = None;
    }
    let handle = adapter.resolve_locator_anchor(&anchor, deadline)?;
    let options = crate::TreeOptions {
        max_depth: 0,
        include_bounds: true,
        skeleton: entry.scope.census_depth.is_some(),
        surface: entry.source.source_surface,
        ..Default::default()
    };
    let hydrated = adapter.observe_tree(
        ObservationRoot::Element {
            handle: &handle,
            entry,
            root_ref: None,
        },
        &ObservationRequest::snapshot(&options, deadline).validate()?,
    )?;
    let live = hydrated
        .roots
        .first()
        .and_then(|index| hydrated.nodes.get(*index as usize))
        .filter(|_| hydrated.roots.len() == 1)
        .ok_or_else(incomplete)?;
    if live.evidence.role.is_unknown() {
        return Err(incomplete());
    }
    if live.evidence.role.known() != Some(&entry.identity.role) {
        return Err(stale());
    }
    if requires_live_uniqueness(&entry.identity) {
        match identity_match(
            entry,
            &live.evidence.name,
            &live.evidence.value,
            &live.evidence.description,
            &live.evidence.identifiers,
        ) {
            IdentityMatch::Match => {}
            IdentityMatch::NoMatch => return Err(stale()),
            IdentityMatch::Unknown => return Err(incomplete()),
        }
    } else if !positional_matches(entry, live) {
        return Err(stale());
    }
    retained_object_verdict(entry, live)?;
    Ok(handle)
}

fn positional_matches(entry: &RefEntry, node: &ObservedNode) -> bool {
    let bounds = match entry.geometry.bounds_space.unwrap_or_default() {
        crate::BoundsSpace::Screen => &node.evidence.ref_evidence.bounds,
        crate::BoundsSpace::Window => &node.evidence.ref_evidence.window_bounds,
    };
    node.evidence.role.known() == Some(&entry.identity.role)
        && entry.geometry.bounds_hash.is_some()
        && bounds.known().and_then(crate::Rect::bounds_hash) == entry.geometry.bounds_hash
}

fn stale() -> AdapterError {
    AdapterError::stale_ref_because("Ref no longer matches inside its source root")
        .with_details(serde_json::json!({ "complete": true }))
}

fn incomplete() -> AdapterError {
    AdapterError::timeout("The ref's source subtree could not be counted completely")
        .with_details(serde_json::json!({ "kind": "scoped_ref_incomplete", "retryable": false }))
}

#[cfg(test)]
#[path = "ref_scope_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "ref_scope_omitted_tests.rs"]
mod omitted_tests;
