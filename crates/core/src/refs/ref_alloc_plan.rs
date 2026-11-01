//! The plan snapshot allocation follows: which nodes receive a ref, and the identity basis each
//! ref records. The census counts every element a live search could match.

use crate::refs::identity_basis_counts::IdentityBasisCounts;
use crate::refs::ref_alloc::{can_receive_observed_ref, identity_from_node};
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
        drill_down: bool,
        complete: bool,
    ) -> Self {
        let mut counts = IdentityBasisCounts::new();
        if drill_down || !complete {
            counts.mark_incomplete();
        }
        Self::count_look_alikes(root, &mut counts);
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

    fn count_look_alikes(node: &AccessibilityNode, counts: &mut IdentityBasisCounts) {
        if node.subtree_truncated || node.children_count.is_some_and(|count| count > 0) {
            counts.mark_incomplete();
        }
        if node.identity.unread.role {
            counts.mark_incomplete();
        }
        mark_unread(counts, node);
        if can_receive_observed_ref(node) {
            counts.record(&identity_from_node(node), &node.identity.native_ids);
        }
        for child in &node.children {
            Self::count_look_alikes(child, counts);
        }
    }
}

/// Withdraws the proofs a node's unread fields could hide a look-alike from. For a
/// mutable-value role, an unread value is an unread name too.
fn mark_unread(counts: &mut IdentityBasisCounts, node: &AccessibilityNode) {
    let unread = node.identity.unread;
    counts.mark_unread(
        &node.role,
        unread.identifiers,
        unread.names || (unread.values && crate::tree::roles::is_mutable_value_role(&node.role)),
        unread.descriptions,
    );
}
