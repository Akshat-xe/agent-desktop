use crate::AccessibilityNode;
use crate::IdentifierKind;
use crate::refs::ref_alloc::is_ref_able;
use crate::refs::ref_full_predicate::FullPredicate;
use std::collections::{HashMap, HashSet};

const NATIVE_ID: u8 = 0b001;
const NAME: u8 = 0b010;
const DESCRIPTION: u8 = 0b100;

/// The structural paths whose full predicate a complete observation found
/// exactly one element for, counted over every observed node rather than over
/// the ref-able ones. A tree with omitted descendants proves nothing.
pub(crate) fn unique_capture_paths(root: &AccessibilityNode) -> HashSet<Vec<usize>> {
    if omits_descendants(root) {
        return HashSet::new();
    }
    let needed = needed_masks(root);
    if needed == 0 {
        return HashSet::new();
    }
    let mut counts = HashMap::new();
    count_buckets(root, needed, &mut counts);
    let mut unique = HashSet::new();
    collect_unique(root, &mut Vec::new(), &counts, &mut unique);
    unique
}

/// Which field subsets a ref-able entry records anywhere in this tree, so the
/// counting pass only tallies the predicates the allocation will look up.
fn needed_masks(node: &AccessibilityNode) -> u8 {
    let mut needed = 0;
    if is_ref_able(node) {
        needed |= 1 << recorded_mask(&FullPredicate::of_node(node));
    }
    for child in &node.children {
        needed |= needed_masks(child);
    }
    needed
}

fn omits_descendants(node: &AccessibilityNode) -> bool {
    node.subtree_truncated
        || node
            .children_count
            .is_some_and(|count| count > 0 && node.children.is_empty())
        || node.children.iter().any(omits_descendants)
}

fn count_buckets<'a>(
    node: &'a AccessibilityNode,
    needed: u8,
    counts: &mut HashMap<PredicateBucket<'a>, usize>,
) {
    let predicate = FullPredicate::of_node(node);
    let recorded = recorded_mask(&predicate);
    let mut mask = recorded;
    loop {
        if needed & (1 << mask) != 0 {
            *counts
                .entry(PredicateBucket::masked(&predicate, mask))
                .or_default() += 1;
        }
        if mask == 0 {
            break;
        }
        mask = (mask - 1) & recorded;
    }
    for child in &node.children {
        count_buckets(child, needed, counts);
    }
}

fn collect_unique<'a>(
    node: &'a AccessibilityNode,
    path: &mut Vec<usize>,
    counts: &HashMap<PredicateBucket<'a>, usize>,
    unique: &mut HashSet<Vec<usize>>,
) {
    let predicate = FullPredicate::of_node(node);
    if is_ref_able(node) && counts.get(&recorded_bucket(&predicate)) == Some(&1) {
        unique.insert(path.clone());
    }
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        collect_unique(child, path, counts, unique);
        path.pop();
    }
}

fn recorded_bucket<'a>(predicate: &FullPredicate<'a>) -> PredicateBucket<'a> {
    PredicateBucket::masked(predicate, recorded_mask(predicate))
}

fn recorded_mask(predicate: &FullPredicate<'_>) -> u8 {
    let mut mask = 0;
    if predicate.native_id.is_some() {
        mask |= NATIVE_ID;
    }
    if predicate.name.is_some() {
        mask |= NAME;
    }
    if predicate.description.is_some() {
        mask |= DESCRIPTION;
    }
    mask
}

/// A predicate narrowed to the fields the entry recorded: an unrecorded field
/// matches any value a candidate presents.
#[derive(Hash, PartialEq, Eq)]
struct PredicateBucket<'a> {
    role: &'a str,
    native_id: Option<(IdentifierKind, &'a str)>,
    name: Option<&'a str>,
    description: Option<&'a str>,
}

impl<'a> PredicateBucket<'a> {
    fn masked(predicate: &FullPredicate<'a>, mask: u8) -> Self {
        Self {
            role: predicate.role,
            native_id: if mask & NATIVE_ID != 0 {
                predicate.native_id
            } else {
                None
            },
            name: if mask & NAME != 0 {
                predicate.name
            } else {
                None
            },
            description: if mask & DESCRIPTION != 0 {
                predicate.description
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
#[path = "ref_alloc_unique_tests.rs"]
mod tests;
