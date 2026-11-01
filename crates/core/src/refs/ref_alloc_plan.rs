//! The plan snapshot allocation follows: which nodes receive a ref, and the identity basis each
//! ref records. The census counts every element a live search could match.

use crate::AccessibilityNode;
use crate::refs::identity_basis_counts::IdentityBasisCounts;
use crate::refs::ref_alloc::{can_receive_observed_ref, identity_from_node};
use std::collections::HashSet;

pub(crate) struct RefAllocPlan {
    skeleton_anchors: HashSet<Vec<usize>>,
    counts: IdentityBasisCounts,
    anchor_counts: IdentityBasisCounts,
}

impl RefAllocPlan {
    pub(crate) fn new(
        root: &AccessibilityNode,
        skeleton_anchors: HashSet<Vec<usize>>,
        complete: bool,
    ) -> Self {
        let mut counts = IdentityBasisCounts::new();
        let mut anchor_counts = IdentityBasisCounts::new();
        if !complete {
            counts.mark_incomplete();
            anchor_counts.mark_incomplete();
        }
        Self::count_look_alikes(root, &mut counts, &mut anchor_counts);
        Self {
            skeleton_anchors,
            counts,
            anchor_counts,
        }
    }

    pub(crate) fn is_skeleton_anchor(&self, path: &[usize]) -> bool {
        self.skeleton_anchors.contains(path)
    }

    /// Records the entry's basis. In a skeleton snapshot, an entry whose look-alikes were
    /// counted only over the observed depth keeps that basis and the depth, so it resolves by
    /// a live count over the same depth instead of window-wide.
    pub(crate) fn record_basis(&self, entry: &mut crate::RefEntry, skeleton_depth: Option<u8>) {
        let basis = self.counts.basis(&entry.identity);
        entry.identity.basis = Some(basis);
        let Some(depth) = skeleton_depth else { return };
        let observed = self.anchor_counts.basis(&entry.identity);
        entry.identity.basis = Some(observed);
        if observed != basis
            && crate::refs::identity_basis_gate::requires_live_uniqueness(&entry.identity)
        {
            entry.scope.census_depth = Some(depth);
        }
    }

    fn count_look_alikes(
        node: &AccessibilityNode,
        counts: &mut IdentityBasisCounts,
        anchor_counts: &mut IdentityBasisCounts,
    ) {
        if node.subtree_truncated || node.children_count.is_some_and(|count| count > 0) {
            counts.mark_incomplete();
        }
        if node.identity.unread.role {
            counts.mark_incomplete();
            anchor_counts.mark_incomplete();
        }
        mark_unread(counts, node);
        mark_unread(anchor_counts, node);
        if can_receive_observed_ref(node) {
            let identity = identity_from_node(node);
            counts.record(&identity, &node.identity.native_ids);
            anchor_counts.record(&identity, &node.identity.native_ids);
        }
        for child in &node.children {
            Self::count_look_alikes(child, counts, anchor_counts);
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
