//! The plan snapshot allocation follows: which nodes receive a ref, and the identity basis each
//! ref records. The census counts every element a live search could match.

use crate::AccessibilityNode;
use crate::IdentityBasis;
use crate::refs::identity_basis_counts::IdentityBasisCounts;
use crate::refs::ref_alloc::{can_receive_observed_ref, identity_from_node, is_ref_able};
use crate::refs::ref_census::Census;
use std::collections::HashSet;

pub(crate) struct RefAllocPlan {
    skeleton_anchors: HashSet<Vec<usize>>,
    counts: IdentityBasisCounts,
    anchor_counts: IdentityBasisCounts,
    census: Option<Census>,
}

impl RefAllocPlan {
    pub(crate) fn new(
        root: &AccessibilityNode,
        plan_anchors: bool,
        complete: bool,
        census: Option<Census>,
    ) -> Self {
        let mut counts = IdentityBasisCounts::new();
        let mut anchor_counts = IdentityBasisCounts::new();
        if !complete {
            counts.mark_incomplete();
            anchor_counts.mark_incomplete();
        }
        Self::count_look_alikes(root, &mut counts, &mut anchor_counts);
        let mut plan = Self {
            skeleton_anchors: HashSet::new(),
            counts,
            anchor_counts,
            census,
        };
        if plan_anchors {
            let mut anchors = HashSet::new();
            plan.plan_skeleton_anchors(root, &mut Vec::new(), &mut anchors);
            plan.skeleton_anchors = anchors;
        }
        plan
    }

    pub(crate) fn is_skeleton_anchor(&self, path: &[usize]) -> bool {
        self.skeleton_anchors.contains(path)
    }

    /// Records the entry's basis. In a snapshot cut by depth, an entry whose look-alikes were
    /// counted only over the observed depth keeps that basis and the census, so it resolves by
    /// a live count that replays the same observation instead of counting window-wide.
    pub(crate) fn record_basis(&self, entry: &mut crate::RefEntry) {
        let (basis, census) = self.basis_and_census(&entry.identity);
        entry.identity.basis = Some(basis);
        entry.scope.census_depth = census;
    }

    fn basis_and_census(
        &self,
        identity: &crate::RefEntryIdentity,
    ) -> (IdentityBasis, Option<Census>) {
        let basis = self.counts.basis(identity);
        let Some(census) = self.census else {
            return (basis, None);
        };
        let observed = self.anchor_counts.basis(identity);
        (
            observed,
            (observed != basis && observed.is_proven()).then_some(census),
        )
    }

    fn is_drillable_anchor(&self, node: &AccessibilityNode) -> bool {
        let identity = identity_from_node(node);
        let (basis, _) = self.basis_and_census(&identity);
        can_receive_observed_ref(node)
            && (basis.is_proven()
                || (basis == IdentityBasis::Positional
                    && node
                        .presentation
                        .bounds
                        .and_then(|bounds| bounds.bounds_hash())
                        .is_some()))
    }

    fn plan_skeleton_anchors(
        &self,
        node: &AccessibilityNode,
        path: &mut Vec<usize>,
        anchors: &mut HashSet<Vec<usize>>,
    ) -> bool {
        let mut uncovered_boundary = node.children_count.is_some();
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            uncovered_boundary |= self.plan_skeleton_anchors(child, path, anchors);
            path.pop();
        }
        if !uncovered_boundary {
            return false;
        }
        if can_receive_observed_ref(node) && is_ref_able(node) {
            return false;
        }
        if !path.is_empty() && self.is_drillable_anchor(node) {
            anchors.insert(path.clone());
            return false;
        }
        true
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
