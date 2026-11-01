use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{
    ElementIdentifier, IdentityMatch, RefEntry,
    locator::live_locator::{
        LocatorEvidence, LocatorField, ObservationSource, ObservedNode, ObservedTree,
    },
    refs::ref_full_predicate::{DESCRIPTION, FullPredicate, LivePredicate, NAME, NATIVE_ID},
    refs::ref_identity::stable_name,
    tree::accname::non_blank,
};

const ROLE: u8 = 0b1000;
const MASKS: usize = 8;

/// One whole-window observation, counted once into the field subsets its
/// entries record, so every tokenless entry published from it is proven by the
/// rule `proves_node` applies rather than by a fresh scan per entry.
///
/// The count is the comparison resolution runs: `FullPredicate::of_entry` of the
/// entry being published, measured against the evidence every node reported, so
/// an identifier chosen by a query or a field that went unread counts the way it
/// will count later.
pub(crate) struct CaptureProof<'a> {
    observed: &'a ObservedTree,
    paths: OnceLock<HashMap<&'a [usize], usize>>,
    buckets: [OnceLock<HashMap<LiveKey<'a>, usize>>; MASKS],
    unread: Vec<(u8, LivePredicate<'a>)>,
    whole_window: bool,
}

impl<'a> CaptureProof<'a> {
    pub(crate) fn new(observed: &'a ObservedTree) -> Self {
        let whole_window = matches!(observed.source, ObservationSource::Window { .. })
            && observed.is_complete()
            && !observed.nodes.iter().any(ObservedNode::omits_descendants);
        Self {
            observed,
            whole_window,
            paths: OnceLock::new(),
            buckets: std::array::from_fn(|_| OnceLock::new()),
            unread: observed
                .nodes
                .iter()
                .filter_map(|node| {
                    let bits = unread_bits(&node.evidence);
                    (bits != 0).then(|| (bits, LivePredicate::observed(&node.evidence)))
                })
                .collect(),
        }
    }

    pub(crate) fn proves(&self, entry: &RefEntry, path: &[usize]) -> bool {
        self.paths
            .get_or_init(|| {
                self.observed
                    .nodes
                    .iter()
                    .enumerate()
                    .map(|(index, node)| (node.path.as_slice(), index))
                    .collect()
            })
            .get(path)
            .is_some_and(|index| self.proves_node(entry, *index))
    }

    /// Whether `entry` was proven unique at the node it was built from: the
    /// observation must be a complete whole-window read, exactly one of its nodes
    /// must match the entry's full predicate, no node may leave a recorded field
    /// unread, and that one node must be this one.
    pub(crate) fn proves_node(&self, entry: &RefEntry, index: usize) -> bool {
        if !self.whole_window {
            return false;
        }
        let predicate = FullPredicate::of_entry(entry);
        let fields = predicate.mask();
        if self.buckets[fields as usize]
            .get_or_init(|| count_nodes(self.observed, fields))
            .get(&key_of(&predicate))
            != Some(&index)
        {
            return false;
        }
        !self.unread.iter().any(|(bits, live)| {
            bits & (fields | ROLE) != 0 && predicate.compare(live) == IdentityMatch::Unknown
        })
    }
}

/// The node indices each full predicate of this field subset matched, as a
/// single index per predicate. A predicate two nodes match is stored with the
/// last of them, so it never equals the node an entry was built from.
fn count_nodes(observed: &ObservedTree, fields: u8) -> HashMap<LiveKey<'_>, usize> {
    let mut counted: HashMap<LiveKey<'_>, (usize, usize)> = HashMap::new();
    for (index, node) in observed.nodes.iter().enumerate() {
        for key in keys_of(&node.evidence, fields) {
            let entry = counted.entry(key).or_insert((0, index));
            entry.0 += 1;
            entry.1 = index;
        }
    }
    counted
        .into_iter()
        .filter(|(_, (count, _))| *count == 1)
        .map(|(key, (_, index))| (key, index))
        .collect()
}

fn keys_of<'a>(evidence: &'a LocatorEvidence, fields: u8) -> Vec<LiveKey<'a>> {
    let Some(role) = non_blank(evidence.role.known().map(String::as_str)) else {
        return Vec::new();
    };
    let name = match fields & NAME {
        0 => None,
        _ => match stable_name(
            role,
            evidence.name.known().map(String::as_str),
            evidence.value.known().map(String::as_str),
        ) {
            Some(name) => Some(name),
            None => return Vec::new(),
        },
    };
    let description = match fields & DESCRIPTION {
        0 => None,
        _ => match non_blank(evidence.description.known().map(String::as_str)) {
            Some(description) => Some(description),
            None => return Vec::new(),
        },
    };
    let key = |native_id: Option<&'a ElementIdentifier>| LiveKey {
        role,
        name,
        description,
        native_id,
    };
    if fields & NATIVE_ID == 0 {
        return vec![key(None)];
    }
    evidence
        .identifiers
        .identifiers()
        .iter()
        .map(|native_id| key(Some(native_id)))
        .collect()
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
