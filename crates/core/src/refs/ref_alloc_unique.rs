use std::collections::{HashMap, HashSet};

use crate::{
    ElementIdentifier, IdentityMatch,
    locator::live_locator::{
        LocatorEvidence, LocatorField, ObservationSource, ObservedNode, ObservedTree,
    },
    refs::ref_full_predicate::{DESCRIPTION, FullPredicate, LivePredicate, NAME, NATIVE_ID},
    refs::ref_identity::stable_name,
    tree::accname::non_blank,
};

const ROLE: u8 = 0b1000;

/// The structural paths whose full predicate a complete observation found
/// exactly one element for, counted over every observed node rather than over
/// the ones a presentation happens to keep. A tree with omitted descendants
/// proves nothing.
///
/// The count is the comparison resolution runs: the predicate a ref will record,
/// measured against the evidence every node reported, so an identifier carried
/// secondhand or a field that went unread counts the way it will count later.
pub(crate) fn unique_capture_paths(observed: &ObservedTree) -> HashSet<Vec<usize>> {
    let Some(root) = observed.roots.first() else {
        return HashSet::new();
    };
    unique_capture_paths_under(observed, *root, &[])
}

/// `unique_capture_paths` for a subtree: the paths are relative to `root`, so a
/// drill-down that allocates from that node can look its own entries up, while
/// the count still runs over every node of the observation.
pub(crate) fn unique_capture_paths_under(
    observed: &ObservedTree,
    root: u32,
    prefix: &[usize],
) -> HashSet<Vec<usize>> {
    if !proves_uniqueness(observed) {
        return HashSet::new();
    }
    let mut candidates = Vec::new();
    let mut path = prefix.to_vec();
    collect_candidates(observed, root, &mut path, &mut candidates);
    found_once(observed, candidates)
}

/// The node indices a complete whole-window observation found exactly once,
/// for refs materialized straight from that observation, such as `find` matches.
/// An element-rooted observation proves nothing about the rest of the window.
pub(crate) fn unique_capture_nodes(observed: &ObservedTree) -> HashSet<usize> {
    let whole_window = matches!(observed.source, ObservationSource::Window { .. });
    if !whole_window || !proves_uniqueness(observed) {
        return HashSet::new();
    }
    let candidates = observed
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.evidence.role.known().is_some())
        .map(|(index, node)| (index, FullPredicate::of_evidence(&node.evidence)))
        .collect();
    found_once(observed, candidates)
}

fn proves_uniqueness(observed: &ObservedTree) -> bool {
    observed.is_complete() && !observed.nodes.iter().any(ObservedNode::omits_descendants)
}

fn found_once<K: Eq + std::hash::Hash>(
    observed: &ObservedTree,
    candidates: Vec<(K, FullPredicate<'_>)>,
) -> HashSet<K> {
    let mut needed = 0_u8;
    for (_, predicate) in &candidates {
        needed |= 1 << predicate.mask();
    }
    if needed == 0 {
        return HashSet::new();
    }
    let census = Census::new(observed, needed);
    candidates
        .into_iter()
        .filter(|(_, predicate)| census.found_once(predicate))
        .map(|(key, _)| key)
        .collect()
}

/// Every node a ref could name, which is any node whose role was read: the
/// ref-able ones and the inert containers that anchor a skeleton alike.
fn collect_candidates<'a>(
    observed: &'a ObservedTree,
    index: u32,
    path: &mut Vec<usize>,
    out: &mut Vec<(Vec<usize>, FullPredicate<'a>)>,
) {
    let Some(node) = observed.nodes.get(index as usize) else {
        return;
    };
    if node.evidence.role.known().is_some() {
        out.push((path.clone(), FullPredicate::of_evidence(&node.evidence)));
    }
    for (position, child) in node.children.iter().enumerate() {
        path.push(position);
        collect_candidates(observed, *child, path, out);
        path.pop();
    }
}

/// The live fields of every observed node, tallied once per field subset the
/// capture will look up, so a predicate's match count is one hash lookup.
struct Census<'a> {
    buckets: HashMap<u8, HashMap<LiveKey<'a>, usize>>,
    unread: Vec<(u8, LivePredicate<'a>)>,
}

impl<'a> Census<'a> {
    fn new(observed: &'a ObservedTree, needed: u8) -> Self {
        let mut buckets: HashMap<u8, HashMap<LiveKey<'a>, usize>> = HashMap::new();
        let mut unread = Vec::new();
        for node in &observed.nodes {
            let mut remaining = needed;
            while remaining != 0 {
                let fields = remaining.trailing_zeros() as u8;
                remaining &= remaining - 1;
                tally(&mut buckets, fields, &node.evidence);
            }
            let bits = unread_bits(&node.evidence);
            if bits != 0 {
                unread.push((bits, LivePredicate::observed(&node.evidence)));
            }
        }
        Self { buckets, unread }
    }

    fn found_once(&self, predicate: &FullPredicate<'_>) -> bool {
        let fields = predicate.mask();
        let matched = self
            .buckets
            .get(&fields)
            .and_then(|bucket| bucket.get(&key_of(predicate)))
            == Some(&1);
        matched
            && !self.unread.iter().any(|(bits, live)| {
                bits & (fields | ROLE) != 0 && predicate.compare(live) == IdentityMatch::Unknown
            })
    }
}

fn tally<'a>(
    buckets: &mut HashMap<u8, HashMap<LiveKey<'a>, usize>>,
    fields: u8,
    evidence: &'a LocatorEvidence,
) {
    let Some(role) = non_blank(evidence.role.known().map(String::as_str)) else {
        return;
    };
    let name = match fields & NAME {
        0 => None,
        _ => match stable_name(
            role,
            evidence.name.known().map(String::as_str),
            evidence.value.known().map(String::as_str),
        ) {
            Some(name) => Some(name),
            None => return,
        },
    };
    let description = match fields & DESCRIPTION {
        0 => None,
        _ => match non_blank(evidence.description.known().map(String::as_str)) {
            Some(description) => Some(description),
            None => return,
        },
    };
    let bucket = buckets.entry(fields).or_default();
    if fields & NATIVE_ID == 0 {
        *bucket
            .entry(LiveKey {
                role,
                name,
                description,
                native_id: None,
            })
            .or_default() += 1;
        return;
    }
    for native_id in evidence.identifiers.identifiers() {
        *bucket
            .entry(LiveKey {
                role,
                name,
                description,
                native_id: Some(native_id),
            })
            .or_default() += 1;
    }
}

/// The fields this element could not answer, so a predicate that records one of
/// them reaches `Unknown` here exactly as it will at resolution.
fn unread_bits(evidence: &LocatorEvidence) -> u8 {
    let live = LivePredicate::observed(evidence);
    let mut bits = 0;
    match live.role() {
        LocatorField::Known(role) => {
            if live.stable_name(role).is_unknown() {
                bits |= NAME;
            }
        }
        LocatorField::Unknown => bits |= ROLE,
        LocatorField::Absent => {}
    }
    if live.description().is_unknown() {
        bits |= DESCRIPTION;
    }
    if !live.native_ids_complete() {
        bits |= NATIVE_ID;
    }
    bits
}

fn key_of<'b>(predicate: &FullPredicate<'b>) -> LiveKey<'b> {
    LiveKey {
        role: predicate.role,
        name: predicate.name,
        description: predicate.description,
        native_id: predicate.native_id,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct LiveKey<'a> {
    role: &'a str,
    name: Option<&'a str>,
    description: Option<&'a str>,
    native_id: Option<&'a ElementIdentifier>,
}

#[cfg(test)]
#[path = "ref_alloc_unique_tests.rs"]
mod tests;
