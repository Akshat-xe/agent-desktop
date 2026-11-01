//! What an enumeration failure means to the uniqueness count: only the end-of-list signal
//! establishes that a child list was read whole, because a unique name is proven only by a
//! complete read of the scope the ref was captured in.

use super::*;
use crate::system::runtime::hresult::{
    E_ACCESSDENIED, E_INVALIDARG, E_POINTER, UIA_E_NOTSUPPORTED,
};
use crate::tree::automation::UiaFailure;
use crate::tree::properties::property_ids::TreeProperty;
use crate::tree::test_support::{number, text};
use crate::tree::walk::walker_fake::{FakeTree, budget, ref_entry};
use agent_desktop_core::IdentityBasis;

const BUTTON: i32 = uiautomation::types::ControlType::Button as i32;

const PANE: i32 = uiautomation::types::ControlType::Pane as i32;

const TARGET_NAME: &str = "Save";

const ROOT: i32 = 1;

const SAVE_A: i32 = 2;

const PANEL: i32 = 3;

const SAVE_B: i32 = 4;

/// The codes a provider can answer a child read with that are not the end-of-list signal: each one
/// is a settled absence in the read table, and none of them is proof that a subtree is empty.
const NOT_ENOUGH_TO_SETTLE: [i32; 3] = [E_POINTER, E_INVALIDARG, UIA_E_NOTSUPPORTED];

fn save_button(tree: FakeTree, node: i32) -> FakeTree {
    tree.reading(
        node,
        &[
            number(TreeProperty::ControlType, BUTTON),
            (TreeProperty::Name, text(TARGET_NAME)),
        ],
    )
}

fn panel(tree: FakeTree, node: i32) -> FakeTree {
    tree.reading(node, &[number(TreeProperty::ControlType, PANE)])
}

/// The finding's shape after the capture: the named Save button the ref was issued for, and a
/// second identically named button inside a panel the search has to read to see.
fn captured_save_beside_a_panel() -> FakeTree {
    let tree = FakeTree::default()
        .with_children(ROOT, &[SAVE_A, PANEL])
        .with_children(PANEL, &[SAVE_B]);
    let tree = panel(tree, ROOT);
    let tree = panel(tree, PANEL);
    save_button(save_button(tree, SAVE_A), SAVE_B)
}

/// Two identically named buttons in one panel's child list, so a sibling step is the only thing
/// between the count and the second match.
fn two_saves_in_one_list() -> FakeTree {
    let tree = FakeTree::default().with_children(ROOT, &[SAVE_A, SAVE_B]);
    save_button(save_button(panel(tree, ROOT), SAVE_A), SAVE_B)
}

/// The stored ref a capture would issue for the first Save button, its role read off the fixture.
fn save_entry(tree: &FakeTree) -> RefEntry {
    let role = tree
        .evidence(&SAVE_A)
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

fn counted_scope(tree: &FakeTree, entry: &RefEntry) -> (Vec<i32>, bool) {
    let budget = budget(10);
    let ctx = SearchContext {
        source: tree,
        entry,
        budget: &budget,
    };
    let mut found = Vec::new();
    let mut unread_region = false;
    search_source_scope(&ctx, &ROOT, 0, &mut found, &mut unread_region)
        .expect("a failed read is not the attempt's answer");
    (found, unread_region)
}

/// A panel that answers its child read with any failure other than the end-of-list signal may
/// still hold a look-alike, so the sole match beside it never resolves. Capture's own walk settles
/// completeness on `is_exhaustion()` alone; the uniqueness count must not settle on less.
#[test]
fn a_panel_whose_first_child_faults_never_settles_a_unique_name() {
    for failure in NOT_ENOUGH_TO_SETTLE {
        let tree = captured_save_beside_a_panel()
            .faulting_on_first_child_with(PANEL, UiaFailure::Hresult(failure));
        let entry = save_entry(&tree);

        let (found, unread_region) = counted_scope(&tree, &entry);

        assert_eq!(
            found,
            vec![SAVE_A],
            "{failure:#x}: the fault stops the walk before the panel's child is counted"
        );
        assert!(
            unread_region,
            "{failure:#x}: an enumeration failure is an unread region, never an empty one"
        );
        assert!(
            matches!(
                classify_identity_search(found.len(), unread_region),
                SearchVerdict::Incomplete
            ),
            "{failure:#x}: one match beside a subtree the walk could not read must never resolve"
        );
    }
}

/// The other direction on the same fixture: the identically named button behind the panel is there
/// to be counted, so a scope read whole settles `AMBIGUOUS_TARGET` rather than a resolution. Without
/// this the case above could pass on a tree that holds no look-alike at all.
#[test]
fn the_same_scope_read_whole_counts_the_look_alike_behind_the_panel() {
    let tree = captured_save_beside_a_panel();
    let entry = save_entry(&tree);

    let (found, unread_region) = counted_scope(&tree, &entry);

    assert_eq!(
        found,
        vec![SAVE_A, SAVE_B],
        "both buttons answer to the recorded role and name"
    );
    assert!(
        !unread_region,
        "every node answered, so nothing in this scope stayed unread"
    );
    assert!(matches!(
        classify_identity_search(found.len(), unread_region),
        SearchVerdict::Ambiguous
    ));
}

/// A sibling step that fails after the first match hides every match behind it, so the list the
/// count saw is a prefix rather than a whole one and the count is withheld.
#[test]
fn a_sibling_fault_after_the_first_match_never_settles_a_unique_name() {
    for failure in NOT_ENOUGH_TO_SETTLE {
        let tree = two_saves_in_one_list()
            .faulting_on_next_sibling_with(SAVE_A, UiaFailure::Hresult(failure));
        let entry = save_entry(&tree);

        let (found, unread_region) = counted_scope(&tree, &entry);

        assert_eq!(
            found,
            vec![SAVE_A],
            "{failure:#x}: the failing sibling walk never yields the second match"
        );
        assert!(
            unread_region,
            "{failure:#x}: the siblings after the fault are unread, not absent"
        );
        assert!(
            matches!(
                classify_identity_search(found.len(), unread_region),
                SearchVerdict::Incomplete
            ),
            "{failure:#x}: one match followed by an unreadable sibling list must never resolve"
        );
    }

    let unfaulted = two_saves_in_one_list();
    let (found, unread_region) = counted_scope(&unfaulted, &save_entry(&unfaulted));
    assert_eq!(
        found,
        vec![SAVE_A, SAVE_B],
        "both siblings are counted when the walk reads the list to its end"
    );
    assert!(matches!(
        classify_identity_search(found.len(), unread_region),
        SearchVerdict::Ambiguous
    ));
}

/// A terminal failure is still the attempt's answer: the count propagates it rather than
/// withholding, because no read this attempt can run would settle the scope.
#[test]
fn a_terminal_enumeration_failure_propagates_instead_of_withholding() {
    let tree = captured_save_beside_a_panel()
        .faulting_on_first_child_with(PANEL, UiaFailure::Hresult(E_ACCESSDENIED));
    let entry = save_entry(&tree);
    let budget = budget(10);
    let ctx = SearchContext {
        source: &tree,
        entry: &entry,
        budget: &budget,
    };
    let mut found = Vec::new();
    let mut unread_region = false;

    let error = search_source_scope(&ctx, &ROOT, 0, &mut found, &mut unread_region)
        .expect_err("a denied read is a refusal, not a count of one");

    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
}
