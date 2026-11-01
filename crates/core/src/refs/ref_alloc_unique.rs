use crate::AccessibilityNode;
use crate::refs::ref_alloc::is_ref_able;
use crate::refs::ref_full_predicate::FullPredicate;
use std::collections::{HashMap, HashSet};

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
        needed |= 1 << FullPredicate::of_node(node).mask();
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
    counts: &mut HashMap<FullPredicate<'a>, usize>,
) {
    let predicate = FullPredicate::of_node(node);
    let recorded = predicate.mask();
    let mut mask = recorded;
    loop {
        if needed & (1 << mask) != 0 {
            *counts.entry(predicate.masked(mask)).or_default() += 1;
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
    counts: &HashMap<FullPredicate<'a>, usize>,
    unique: &mut HashSet<Vec<usize>>,
) {
    let predicate = FullPredicate::of_node(node);
    if is_ref_able(node) && counts.get(&predicate) == Some(&1) {
        unique.insert(path.clone());
    }
    for (index, child) in node.children.iter().enumerate() {
        path.push(index);
        collect_unique(child, path, counts, unique);
        path.pop();
    }
}

#[cfg(test)]
#[path = "ref_alloc_unique_tests.rs"]
mod tests;
