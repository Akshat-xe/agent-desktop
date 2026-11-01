use crate::{
    AdapterError, CommandContext, Deadline, IdentityBasis, NativeHandle, PlatformAdapter, RefEntry,
    refs::RefMap, refs::identity_basis_counts::IdentityBasisCounts,
    refs::identity_basis_gate::basis_gate, refs::refs_store::RefStore,
};

pub(crate) fn resolve(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    context: &CommandContext,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    if entry.scope.root_ref.is_none() {
        return adapter.resolve_element_strict(entry, deadline);
    }
    basis_gate(&entry.identity)?;
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

pub(crate) fn resolve_drill_root(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    map: &RefMap,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    resolve_in_map(adapter, &root_probe(entry), map, deadline)
}

fn resolve_in_map(
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
        ancestors.push(root_probe(root));
        current = root;
    }
    let mut roots = ancestors.iter().rev();
    let Some(mut root) = roots.next() else {
        return adapter.resolve_element_strict(entry, deadline);
    };
    let mut handle = adapter.resolve_element_strict(root, deadline)?;
    for target in roots.chain(std::iter::once(entry)) {
        basis_gate(&target.identity)?;
        handle = crate::locator::live_locator::ref_scope::resolve_in_root(
            adapter, target, root, &handle, deadline,
        )?;
        root = target;
    }
    for ancestor in ancestors.iter().skip(1) {
        crate::locator::live_locator::ref_scope::pin_root(adapter, ancestor, deadline)?;
    }
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    Ok(handle)
}

fn root_probe(entry: &RefEntry) -> RefEntry {
    let mut probe = entry.clone();
    if probe.identity.basis == Some(IdentityBasis::Ambiguous) {
        let mut fields = IdentityBasisCounts::new();
        fields.record(&probe.identity, &[]);
        let basis = fields.basis(&probe.identity);
        if basis != IdentityBasis::Positional {
            probe.identity.basis = Some(basis);
        }
    }
    probe
}
