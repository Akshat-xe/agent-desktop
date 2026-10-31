//! The plan snapshot allocation follows: which nodes receive a ref, and the
//! identity basis each of those refs records.

use crate::refs::identity_basis_counts::IdentityBasisCounts;
use crate::refs::ref_alloc::{identity_from_node, is_ref_able};
use crate::{AccessibilityNode, IdentityBasis, RefEntryIdentity};
use std::collections::HashSet;

pub(crate) struct RefAllocPlan {
    skeleton_anchors: HashSet<Vec<usize>>,
    counts: IdentityBasisCounts,
}

impl RefAllocPlan {
    pub(crate) fn new(
        root: &AccessibilityNode,
        skeleton_anchors: HashSet<Vec<usize>>,
        path_prefix: &[usize],
        drill_down: bool,
        complete: bool,
    ) -> Self {
        let mut counts = IdentityBasisCounts::new();
        if drill_down || !complete {
            counts.mark_incomplete();
        }
        let mut path = path_prefix.to_vec();
        Self::count_look_alikes(root, &mut path, &skeleton_anchors, &mut counts);
        Self {
            skeleton_anchors,
            counts,
        }
    }

    pub(crate) fn is_skeleton_anchor(&self, path: &[usize]) -> bool {
        self.skeleton_anchors.contains(path)
    }

    pub(crate) fn basis(&self, identity: &RefEntryIdentity) -> IdentityBasis {
        self.counts.basis(identity)
    }

    fn count_look_alikes(
        node: &AccessibilityNode,
        path: &mut Vec<usize>,
        anchors: &HashSet<Vec<usize>>,
        counts: &mut IdentityBasisCounts,
    ) {
        if node.subtree_truncated || node.children_count.is_some_and(|count| count > 0) {
            counts.mark_incomplete();
        }
        if node.identity.unread.role {
            counts.mark_incomplete();
        }
        mark_unread(counts, node);
        if receives_ref(node, path, anchors) {
            counts.record(&identity_from_node(node), &node.identity.native_ids);
        }
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            Self::count_look_alikes(child, path, anchors, counts);
            path.pop();
        }
    }
}

/// Withdraws the proofs a node's unread fields could hide a look-alike from. A
/// mutable-value role's stable name rests on its value, so an unread value is
/// an unread name there too.
fn mark_unread(counts: &mut IdentityBasisCounts, node: &AccessibilityNode) {
    let unread = node.identity.unread;
    counts.mark_unread(
        &node.role,
        unread.identifiers,
        unread.names || (unread.values && crate::tree::roles::is_mutable_value_role(&node.role)),
        unread.descriptions,
    );
}

/// Mirrors the allocation itself: a node is an entry when it is ref-able or a
/// planned skeleton anchor, and allocation drops one whose role cannot be
/// addressed, so it is not a look-alike either.
fn receives_ref(node: &AccessibilityNode, path: &[usize], anchors: &HashSet<Vec<usize>>) -> bool {
    (is_ref_able(node) || anchors.contains(path))
        && crate::Role::is_canonical(&node.role)
        && node.role != "unknown"
}
