//! The scope the identity count searches: how deep it descends, and when it has
//! read enough to stop.

use super::*;
use crate::tree::walk::walker_fake::{FakeTree, budget, walk};

/// The snapshot walker visits the node at its raw-depth ceiling and cuts only
/// the children below it, so a ref can name an element exactly at
/// `MAX_RESOLVE_DEPTH`. The count must evaluate that node and cut at the same
/// level: stopping one level earlier leaves a boundary ref unsearchable, and the
/// boundary node's child then flags the scope unread, so even a unique target
/// found above it is refused as incomplete on every attempt.
#[test]
fn the_count_reaches_the_deepest_level_the_snapshot_walker_emits() {
    let deepest = MAX_RESOLVE_DEPTH;
    let chain: Vec<i32> = (1..=i32::from(deepest) + 2).collect();
    let tree = FakeTree::default().with_chain(&chain);
    let outcome = walk(&tree, budget(deepest));

    assert_eq!(
        outcome.stats.traversal.max_raw_depth, deepest,
        "the walker visits the node at its ceiling, so the snapshot can issue a ref there"
    );
    assert!(
        !at_deepest_searched_level(deepest - 1),
        "the parent of that node must still descend into it"
    );
    assert!(
        at_deepest_searched_level(deepest),
        "the count cuts where the walker cuts, one level below the deepest emitted node"
    );
}

/// Two matches settle the verdict: the classification answers `AMBIGUOUS_TARGET`
/// for any count above one, whatever the rest of the scope holds, and this tier
/// has no bounds tie-break to separate them. The walk stops there instead of
/// reading the remaining tree, so the count it reports is the one it stopped at.
#[test]
fn a_second_match_ends_the_search_with_the_verdict_a_full_count_would_give() {
    assert!(!match_count_is_conclusive(0));
    assert!(
        !match_count_is_conclusive(1),
        "one match is exactly the count a look-alike elsewhere could overturn"
    );
    assert!(
        match_count_is_conclusive(2),
        "a second match ends the walk: nothing it never read could make it one"
    );
    assert!(
        matches!(classify_identity_search(1, true), SearchVerdict::Incomplete),
        "one match must keep reading, so the walk stops strictly above it"
    );
    assert!(matches!(
        classify_identity_search(2, true),
        SearchVerdict::Ambiguous
    ));
}
