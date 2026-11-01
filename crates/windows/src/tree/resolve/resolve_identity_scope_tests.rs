//! The scope the identity count searches: how deep it descends, and when it has
//! read enough to stop.

use super::*;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::test_support::{number, text};
use crate::tree::walk::walker_fake::{FakeTree, budget, ref_entry, walk};
use agent_desktop_core::IdentityBasis;

const BUTTON: i32 = uiautomation::types::ControlType::Button as i32;

const GROUP: i32 = uiautomation::types::ControlType::Group as i32;

const TARGET_NAME: &str = "Save";

/// The snapshot walker visits the node at its raw-depth ceiling and cuts only the children below
/// it, so the count must evaluate a node at exactly `MAX_RESOLVE_DEPTH` and cut at the same level.
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

fn named_button(tree: FakeTree, node: i32, name: &str) -> FakeTree {
    tree.reading(
        node,
        &[
            number(TreeProperty::ControlType, BUTTON),
            (TreeProperty::Name, text(name)),
        ],
    )
}

/// Seeds every ancestor of `target` with a known control type that cannot match the stored role,
/// because an unseeded node answers every property `Unknown` and would make the region unread for a
/// reason unrelated to the depth under test.
fn grouped_spine(tree: FakeTree, target: i32) -> FakeTree {
    (1..target).fold(tree, |tree, node| {
        tree.reading(node, &[number(TreeProperty::ControlType, GROUP)])
    })
}

/// The stored ref a capture would issue for `node`: its role read off the
/// fixture rather than assumed, and the one stable name these fixtures record.
fn name_entry(tree: &FakeTree, node: &i32) -> RefEntry {
    let role = tree
        .evidence(node)
        .1
        .role
        .known()
        .cloned()
        .expect("the fixture node publishes a role");
    let mut entry = ref_entry(&role);
    entry.identity.basis = Some(IdentityBasis::Name);
    entry.identity.name = Some(TARGET_NAME.to_string());
    entry
}

/// Runs the shipped count traversal over a fixture root, so a case asserts what
/// the walk itself does rather than restating its decisions.
fn walked_scope(tree: &FakeTree, entry: &RefEntry) -> Result<(Vec<i32>, bool), AdapterError> {
    let budget = budget(10);
    let ctx = SearchContext {
        source: tree,
        entry,
        budget: &budget,
    };
    let mut found = Vec::new();
    let mut unread_region = false;
    search_source_scope(&ctx, &1, 0, &mut found, &mut unread_region)?;
    Ok((found, unread_region))
}

/// The traversal, not just the depth helper, must evaluate the node at the snapshot's depth
/// boundary; stopping one level earlier turns one match into none plus an unread region. Every
/// ancestor publishes a known role that cannot match the stored one, so the depth rule is the only
/// thing this case can fail on.
#[test]
fn the_count_traversal_evaluates_the_node_at_the_snapshot_depth_boundary() {
    let target = i32::from(MAX_RESOLVE_DEPTH) + 1;
    let spine: Vec<i32> = (1..=target).collect();
    let tree = FakeTree::default().with_chain(&spine);
    let tree = grouped_spine(tree, target);
    let tree = named_button(tree, target, TARGET_NAME);
    let entry = name_entry(&tree, &target);

    let (found, unread_region) =
        walked_scope(&tree, &entry).expect("a spine of leaves answers without surfacing");

    assert_eq!(
        found,
        vec![target],
        "the deepest node the snapshot can issue a ref for is the one the count finds"
    );
    assert!(
        !unread_region,
        "a spine that ends at the boundary leaves nothing unread"
    );
    assert!(matches!(
        classify_identity_search(found.len(), unread_region),
        SearchVerdict::Resolved(0)
    ));
}

/// Two matches settle `AMBIGUOUS_TARGET` whatever the rest of the scope holds, so the walk stops
/// there and reports the count it stopped at.
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

/// The second match ends the walk before this node's own depth probe or child read, so a failing
/// read beneath it cannot replace the proven ambiguity with an unrelated error.
#[test]
fn a_second_match_stops_before_reading_the_subtree_beneath_it() {
    let tree = FakeTree::default().with_children(1, &[2, 3]);
    let tree = named_button(tree, 2, TARGET_NAME);
    let tree = named_button(tree, 3, TARGET_NAME).faulting_on_first_child(3);
    let entry = name_entry(&tree, &2);

    let (found, unread_region) = walked_scope(&tree, &entry)
        .expect("a second match settles the verdict before any read beneath it");

    assert_eq!(
        found,
        vec![2, 3],
        "both live controls answer to the recorded identity"
    );
    assert!(matches!(
        classify_identity_search(found.len(), unread_region),
        SearchVerdict::Ambiguous
    ));
}

/// The guard is proof of a duplicate, not of a match: a subtree unread beneath the first match can
/// still hide a look-alike, so that failure reaches the caller.
#[test]
fn a_first_match_keeps_reading_the_subtree_beneath_it() {
    let tree = FakeTree::default().with_children(1, &[2]);
    let tree = named_button(tree, 2, TARGET_NAME).faulting_on_first_child(2);
    let entry = name_entry(&tree, &2);

    let error = walked_scope(&tree, &entry)
        .expect_err("an unreadable subtree under the sole match never settles the count");

    assert!(
        !matches!(error.code, agent_desktop_core::ErrorCode::AmbiguousTarget),
        "one match plus an unread region is not a duplicate: {}",
        error.message
    );
}
