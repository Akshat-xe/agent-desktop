use super::{LocatorEvidence, LocatorField, ObservationSource, ObservedNode, ObservedTree};
use crate::{
    RefEntryIdentity,
    adapter::SnapshotSurface,
    locator::LocatorQuery,
    refs::identity_basis_counts::{IdentityBasisCounts, unproven_basis},
    refs::ref_alloc::is_ref_able_role_actions,
    refs::{RefEntry, RefMap, RefPath},
};

/// Counts look-alikes among every role-readable observed element, addressable or not,
/// so each match learns which of its fields is unique. A partial survey proves nothing, and
/// an element-rooted survey proves uniqueness only inside a root that scoped refs resolve under.
pub(crate) fn survey_counts(tree: &ObservedTree, query: &LocatorQuery) -> IdentityBasisCounts {
    let mut counts = IdentityBasisCounts::new();
    let scoped = match &tree.source {
        ObservationSource::Window { .. } => true,
        ObservationSource::Element { root_ref, .. } => root_ref.is_some(),
    };
    if !tree.structurally_complete || tree.omits_descendants() || !scoped {
        counts.mark_incomplete();
    }
    for node in &tree.nodes {
        mark_unread_fields(&mut counts, &node.evidence);
        if node.evidence.role.known().is_none() {
            continue;
        }
        counts.record(
            &ref_identity(&node.evidence, query),
            node.evidence.identifiers.identifiers(),
        );
    }
    counts
}

pub(crate) fn materialize_refmap(
    tree: &mut ObservedTree,
    query: &LocatorQuery,
    counts: &IdentityBasisCounts,
    selected: Option<&[usize]>,
) -> Result<(RefMap, bool), crate::AdapterError> {
    let source = &tree.source;
    let nodes = &mut tree.nodes;
    let mut refmap = RefMap::new();
    let mut complete = true;
    let indices = selected
        .map(|indices| indices.to_vec())
        .unwrap_or_else(|| (0..nodes.len()).collect());
    for index in indices {
        let node = &nodes[index];
        let (addressable, known) = addressability(&node.evidence);
        complete &= known;
        if !addressable {
            continue;
        }
        let entry = based_ref_entry(node, source, query, counts);
        let ref_id = refmap.try_allocate(entry).map_err(|error| {
            crate::AdapterError::new(crate::ErrorCode::InvalidArgs, error.to_string())
        })?;
        nodes[index].ref_id = Some(ref_id);
    }
    Ok((refmap, complete))
}

/// Builds the node's ref entry with the basis the survey proved for it.
pub(crate) fn based_ref_entry(
    node: &ObservedNode,
    source: &ObservationSource,
    query: &LocatorQuery,
    counts: &IdentityBasisCounts,
) -> RefEntry {
    let mut entry = ref_entry(node, source, query);
    let basis = counts.basis(&entry.identity);
    entry.identity.basis = Some(basis);
    crate::refs::ref_identity::withhold_unverified_name(
        &mut entry.identity,
        node.evidence.value.is_unknown(),
        Some(basis),
    );
    entry
}

/// Withdraws the proofs that rest on a field this element left unread. A value
/// matters only where a mutable-value role's stable name depends on it.
fn mark_unread_fields(counts: &mut IdentityBasisCounts, evidence: &LocatorEvidence) {
    let Some(role) = evidence.role.known() else {
        counts.mark_incomplete();
        return;
    };
    let value_decides_name = crate::tree::roles::is_mutable_value_role(role);
    counts.mark_unread(
        role,
        !evidence.identifiers.is_complete(),
        evidence.name.is_unknown() || (value_decides_name && evidence.value.is_unknown()),
        evidence.description.is_unknown(),
    );
}

pub(crate) fn addressability(evidence: &LocatorEvidence) -> (bool, bool) {
    let Some(role) = evidence.role.known() else {
        return (false, false);
    };
    match &evidence.ref_evidence.available_actions {
        LocatorField::Known(actions) => (is_ref_able_role_actions(role, actions), true),
        LocatorField::Absent => (is_ref_able_role_actions(role, &[]), true),
        LocatorField::Unknown => (is_ref_able_role_actions(role, &[]), false),
    }
}

/// Whether every field a ref entry and a match response carry was read, none
/// of them unknown. An observation of that kind leaves nothing for a second
/// read of the same node to add.
pub(crate) fn fully_observed(evidence: &LocatorEvidence, space: crate::BoundsSpace) -> bool {
    !evidence.role.is_unknown()
        && !evidence.name.is_unknown()
        && !evidence.description.is_unknown()
        && !evidence.value.is_unknown()
        && evidence.identifiers.is_complete()
        && !evidence.states.is_unknown()
        && super::hydrate::geometry_complete(&evidence.ref_evidence, space)
        && addressability(evidence).1
}

pub(crate) fn screen_bounds(evidence: &LocatorEvidence) -> Option<crate::Rect> {
    evidence.ref_evidence.bounds.known().copied()
}

fn ref_identity(evidence: &LocatorEvidence, query: &LocatorQuery) -> RefEntryIdentity {
    let mut identity = RefEntryIdentity {
        basis: None,
        retained_object: evidence.identifiers.retained_object().map(str::to_owned),
        role: evidence
            .role
            .known()
            .cloned()
            .unwrap_or_else(|| "unknown".into()),
        name: evidence.name.meaningful_string(),
        value: evidence.value.meaningful_string(),
        description: evidence.description.meaningful_string(),
        native_id: selected_identifier(evidence, query.identity.native_id.as_deref()),
    };
    identity.basis = Some(unproven_basis(&identity));
    identity
}

pub(crate) fn ref_entry(
    node: &super::ObservedNode,
    source: &ObservationSource,
    query: &LocatorQuery,
) -> RefEntry {
    let window = node.evidence.ref_evidence.window_bounds.known().copied();
    let bounds = window.or_else(|| node.evidence.ref_evidence.bounds.known().copied());
    let mut entry = RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(0),
            process_instance: None,
        },
        identity: ref_identity(&node.evidence, query),
        geometry: crate::RefGeometry {
            bounds,
            bounds_hash: bounds.as_ref().and_then(|value| value.bounds_hash()),
            bounds_space: window.map(|_| crate::BoundsSpace::Window),
        },
        capabilities: crate::RefCapabilities {
            states: node.evidence.states.known().cloned().unwrap_or_default(),
            available_actions: available_actions(&node.evidence),
        },
        source: crate::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: crate::RefScope {
            root_ref: None,
            path_is_absolute: false,
            census_depth: None,
            path: node.path.clone(),
        },
    };
    apply_source(&mut entry, source, &node.path);
    entry
}

fn apply_source(entry: &mut RefEntry, source: &ObservationSource, node_path: &RefPath) {
    match source {
        ObservationSource::Window { window, surface } => {
            entry.process.pid = window.pid;
            entry.source.source_app = Some(window.app.clone());
            entry.source.source_window_id = Some(window.id.clone());
            entry.source.source_window_title = Some(window.title.clone());
            entry.source.source_window_bounds_hash =
                window.bounds.as_ref().and_then(crate::Rect::bounds_hash);
            entry.source.source_surface = *surface;
            entry.process.process_instance = window.process_instance.clone();
        }
        ObservationSource::Element {
            entry: source_entry,
            root_ref,
        } => {
            entry.process = source_entry.process.clone();
            entry.source = source_entry.source.clone();
            entry.scope.root_ref = root_ref
                .clone()
                .or_else(|| source_entry.scope.root_ref.clone());
            entry.scope.path_is_absolute =
                entry.scope.root_ref.is_some() || source_entry.scope.path_is_absolute;
            let mut path = source_entry.scope.path.clone();
            path.extend(node_path.iter().copied());
            entry.scope.path = path;
        }
    }
}

fn selected_identifier(
    evidence: &LocatorEvidence,
    expected: Option<&str>,
) -> Option<crate::ElementIdentifier> {
    if let Some(expected) = expected {
        if let Some(preferred) = evidence.identifiers.preferred_identifier()
            && preferred.value == expected
        {
            return Some(preferred.clone());
        }
        return evidence
            .identifiers
            .identifiers()
            .iter()
            .find(|identifier| identifier.value == expected)
            .cloned();
    }
    evidence.identifiers.preferred_identifier().cloned()
}

fn available_actions(evidence: &LocatorEvidence) -> Vec<String> {
    match &evidence.ref_evidence.available_actions {
        LocatorField::Known(actions) => actions.clone(),
        LocatorField::Absent | LocatorField::Unknown => Vec::new(),
    }
}
