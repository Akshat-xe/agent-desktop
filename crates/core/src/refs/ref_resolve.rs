use crate::{
    AdapterError, CommandContext, Deadline, NativeHandle, PlatformAdapter, RefEntry, refs::RefMap,
    refs::identity_basis_gate::basis_gate, refs::refs_store::RefStore,
};

/// Resolves a saved ref to its live element, running the identity-basis gate first. Every
/// ref-resolving caller goes through here.
pub fn resolve(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    context: &CommandContext,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    basis_gate(&entry.identity)?;
    if entry.scope.root_ref.is_none() {
        return resolve_unscoped(adapter, entry, deadline);
    }
    let (snapshot, local) = crate::refs::ref_token::resolve_ref_target(ref_id)
        .map_err(|_| AdapterError::stale_ref_because("Scoped ref has no source snapshot"))?;
    let map = RefStore::for_session(context.session_id())
        .and_then(|store| store.load_snapshot(&snapshot))
        .map_err(|_| {
            AdapterError::stale_ref_because("Scoped ref's source snapshot is unavailable")
        })?;
    if map.get(&local) != Some(entry) {
        return Err(AdapterError::stale_ref_because(
            "Scoped ref no longer belongs to its source snapshot",
        ));
    }
    resolve_in_map(adapter, entry, &map, deadline)
}

/// Points a stale scoped ref at re-drilling its root, since a moved root is never followed.
pub(crate) fn redrill(error: AdapterError) -> AdapterError {
    if error.code == crate::ErrorCode::StaleRef {
        return error.with_suggestion("Drill the root ref again (or re-run the find), then retry.");
    }
    error
}

fn resolve_unscoped(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    match entry.scope.census_depth {
        Some(census) => crate::locator::live_locator::ref_scope::resolve_by_census(
            adapter, entry, census, deadline,
        ),
        None => adapter.resolve_element_strict(entry, deadline),
    }
}

pub(crate) fn resolve_in_map(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    map: &RefMap,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    basis_gate(&entry.identity)?;
    let mut ancestors = Vec::new();
    let mut current = entry;
    let mut visited = std::collections::HashSet::new();
    while let Some(root_ref) = current.scope.root_ref.as_deref() {
        if !visited.insert(root_ref) {
            return Err(AdapterError::stale_ref_because(
                "Scoped ref contains a root cycle",
            ));
        }
        let root = map.get(root_ref).ok_or_else(|| {
            AdapterError::stale_ref_because("Scoped ref's root is absent from its source snapshot")
        })?;
        if root.process != entry.process || root.source != entry.source {
            return Err(AdapterError::stale_ref_because(
                "Scoped ref's root has a different source",
            ));
        }
        ancestors.push(root.clone());
        current = root;
    }
    let mut roots = ancestors.iter().rev();
    let Some(mut root) = roots.next() else {
        return resolve_unscoped(adapter, entry, deadline);
    };
    let mut handle = resolve_unscoped(adapter, root, deadline).map_err(redrill)?;
    for target in roots.chain(std::iter::once(entry)) {
        basis_gate(&target.identity)?;
        handle = crate::locator::live_locator::ref_scope::resolve_in_root(
            adapter, target, root, &handle, deadline,
        )
        .map_err(redrill)?;
        root = target;
    }
    for ancestor in ancestors.iter().skip(1) {
        crate::locator::live_locator::ref_scope::pin_root(adapter, ancestor, deadline)
            .map_err(redrill)?;
    }
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    Ok(handle)
}
