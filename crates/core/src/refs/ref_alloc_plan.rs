//! The plan snapshot allocation follows: which nodes receive a ref, and the
//! identity basis each of those refs records.

use crate::refs::identity_basis_counts::IdentityBasisCounts;
use crate::refs::ref_alloc::{identity_from_node, is_ref_able};
use crate::{AccessibilityNode, IdentityBasis};
use std::collections::{HashMap, HashSet};

pub(crate) struct RefAllocPlan {
    skeleton_anchors: HashSet<Vec<usize>>,
    bases: HashMap<Vec<usize>, IdentityBasis>,
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
        let mut bases = HashMap::new();
        let mut path = path_prefix.to_vec();
        Self::record_bases(root, &mut path, &skeleton_anchors, &counts, &mut bases);
        Self {
            skeleton_anchors,
            bases,
        }
    }

    pub(crate) fn is_skeleton_anchor(&self, path: &[usize]) -> bool {
        self.skeleton_anchors.contains(path)
    }

    pub(crate) fn basis(&self, path: &[usize]) -> Option<IdentityBasis> {
        self.bases.get(path).copied()
    }

    fn count_look_alikes(
        node: &AccessibilityNode,
        path: &mut Vec<usize>,
        anchors: &HashSet<Vec<usize>>,
        counts: &mut IdentityBasisCounts,
    ) {
        if node.subtree_truncated {
            counts.mark_incomplete();
        }
        if receives_ref(node, path, anchors) {
            counts.record(&identity_from_node(node));
        }
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            Self::count_look_alikes(child, path, anchors, counts);
            path.pop();
        }
    }

    fn record_bases(
        node: &AccessibilityNode,
        path: &mut Vec<usize>,
        anchors: &HashSet<Vec<usize>>,
        counts: &IdentityBasisCounts,
        bases: &mut HashMap<Vec<usize>, IdentityBasis>,
    ) {
        if receives_ref(node, path, anchors) {
            let identity = identity_from_node(node);
            bases.insert(path.clone(), counts.basis(&identity));
        }
        for (index, child) in node.children.iter().enumerate() {
            path.push(index);
            Self::record_bases(child, path, anchors, counts, bases);
            path.pop();
        }
    }
}

/// Mirrors the allocation itself: a node is an entry when it is ref-able or a
/// planned skeleton anchor, and allocation drops one whose role cannot be
/// addressed, so it is not a look-alike either.
fn receives_ref(node: &AccessibilityNode, path: &[usize], anchors: &HashSet<Vec<usize>>) -> bool {
    (is_ref_able(node) || anchors.contains(path))
        && crate::Role::is_canonical(&node.role)
        && node.role != "unknown"
}
