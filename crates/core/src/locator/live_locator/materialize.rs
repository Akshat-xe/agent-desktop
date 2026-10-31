use super::{LocatorEvidence, LocatorField, ObservationSource, ObservedTree};
use crate::{
    adapter::SnapshotSurface,
    locator::LocatorQuery,
    refs::identity_basis_counts::{IdentityBasisCounts, unproven_basis},
    refs::ref_alloc::is_ref_able_role_actions,
    refs::{RefEntry, RefMap, RefPath},
};

/// Counts the look-alikes among every addressable element the locator
/// observed, so each match can learn which of its fields is unique.
pub(crate) fn survey_counts(tree: &ObservedTree, query: &LocatorQuery) -> IdentityBasisCounts {
    let mut counts = IdentityBasisCounts::new();
    if !tree.structurally_complete {
        counts.mark_incomplete();
    }
    for node in &tree.nodes {
        let (addressable, _) = addressability(&node.evidence);
        if !addressable {
            continue;
        }
        mark_unread_fields(&mut counts, &node.evidence);
        counts.record(&ref_entry(node, &tree.source, query).identity);
    }
    counts
}

pub(crate) fn materialize_refmap(
    tree: &mut ObservedTree,
    query: &LocatorQuery,
    selected: Option<&[usize]>,
) -> Result<(RefMap, bool), crate::AdapterError> {
    let counts = survey_counts(tree, query);
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
        let mut entry = ref_entry(node, source, query);
        entry.identity.basis = Some(counts.basis(&entry.identity));
        let ref_id = refmap.try_allocate(entry).map_err(|error| {
            crate::AdapterError::new(crate::ErrorCode::InvalidArgs, error.to_string())
        })?;
        nodes[index].ref_id = Some(ref_id);
    }
    Ok((refmap, complete))
}

/// Withdraws the uniqueness proofs that rest on a field this element left
/// unread, since it could be a look-alike the survey could not compare. A value
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

pub(crate) fn ref_entry(
    node: &super::ObservedNode,
    source: &ObservationSource,
    query: &LocatorQuery,
) -> RefEntry {
    let role = node
        .evidence
        .role
        .known()
        .cloned()
        .unwrap_or_else(|| "unknown".into());
    let window = node.evidence.ref_evidence.window_bounds.known().copied();
    let bounds = window.or_else(|| node.evidence.ref_evidence.bounds.known().copied());
    let identifier = selected_identifier(&node.evidence, query.identity.native_id.as_deref());
    let mut entry = RefEntry {
        process: crate::RefProcess {
            pid: crate::ProcessId::new(0),
            process_instance: None,
        },
        identity: crate::RefEntryIdentity {
            basis: None,
            retained_object: node
                .evidence
                .identifiers
                .retained_object()
                .map(str::to_owned),
            role: role.clone(),
            name: node.evidence.name.meaningful_string(),
            value: node.evidence.value.meaningful_string(),
            description: node.evidence.description.meaningful_string(),
            native_id: identifier,
        },
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
            path: node.path.clone(),
        },
    };
    entry.identity.basis = Some(unproven_basis(&entry.identity));
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
