use agent_desktop_core::{DeliverySemantics, ErrorCode, IdentityBasis, RefEntry};

use super::*;
use crate::tree::Field;
use crate::tree::resolve::resolve_errors::window_replaced;
use crate::tree::resolve::resolve_fake::{FRAME_PATH, Fake, deadline, node};
use crate::tree::vocabulary::role_numbers::{PANEL, PUSH_BUTTON};

fn kind(error: &AdapterError) -> &str {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
}

fn candidate_count(error: &AdapterError) -> u64 {
    error.details.as_ref().expect("details")["candidate_count"]
        .as_u64()
        .expect("a candidate count")
}

fn one_button() -> Fake {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/back", node(PUSH_BUTTON, "Back", 30, 30));
    fake
}

fn named_twins() -> Fake {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/first", node(PUSH_BUTTON, "Save", 80, 20));
    fake.add(FRAME_PATH, "/second", node(PUSH_BUTTON, "Save", 200, 20));
    fake
}

fn resolved(fake: &mut Fake, entry: &RefEntry) -> String {
    resolve(fake, entry, deadline())
        .expect("the ref resolves")
        .path()
        .to_owned()
}

fn refusal(fake: &mut Fake, entry: &RefEntry) -> AdapterError {
    resolve(fake, entry, deadline()).expect_err("the ref is refused")
}

#[test]
fn a_ref_written_before_bases_existed_is_refused_before_any_live_search() {
    let mut fake = one_button();
    let mut entry = fake.entry_of("/back", true);
    entry.identity.basis = None;
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(fake.reads.gate, 1);
    assert_eq!(fake.reads.levels, 0);
    assert_eq!(fake.reads.searches, 0);
}

#[test]
fn an_ambiguous_basis_is_refused_before_any_live_search() {
    let mut fake = named_twins();
    let mut entry = fake.entry_of("/first", true);
    entry.identity.basis = Some(IdentityBasis::Ambiguous);
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(kind(&error), "identity_not_unique");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(fake.reads.levels, 0);
    assert_eq!(fake.reads.searches, 0);
}

#[test]
fn the_window_gate_answers_before_a_missing_basis_does() {
    let mut fake = one_button();
    let mut entry = fake.entry_of("/back", true);
    entry.identity.basis = None;
    fake.faults.gate_error = Some(window_replaced());
    let error = refusal(&mut fake, &entry);
    assert_eq!(kind(&error), "resolve_window_replaced");
}

#[test]
fn a_name_basis_ref_resolves_only_when_exactly_one_live_node_carries_it() {
    let mut fake = named_twins();
    let mut entry = fake.entry_of("/first", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::Name));
    entry.scope.path.clear();
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(candidate_count(&error), 2);

    fake.nodes
        .get_mut("/second")
        .expect("second")
        .read
        .node
        .properties
        .name = Field::Present("Cancel".to_owned());
    assert_eq!(resolved(&mut fake, &entry), "/first");
}

#[test]
fn a_name_basis_ref_is_stale_when_no_live_node_carries_its_name() {
    let mut fake = named_twins();
    let mut entry = fake.entry_of("/first", true);
    entry.scope.path.clear();
    for path in ["/first", "/second"] {
        fake.nodes
            .get_mut(path)
            .expect("twin")
            .read
            .node
            .properties
            .name = Field::Present("Close".to_owned());
    }
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn stored_bounds_never_pick_between_two_nodes_that_carry_the_refs_name() {
    let mut fake = named_twins();
    let mut entry = fake.entry_of("/second", true);
    assert_eq!(
        entry.geometry.bounds.map(|bounds| bounds.width),
        Some(200.0)
    );
    assert_eq!(entry.scope.path.as_slice(), [1]);
    entry.scope.path.clear();
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(candidate_count(&error), 2);
}

#[test]
fn bounds_never_confirm_a_lone_name_match() {
    let mut fake = one_button();
    let entry = fake.entry_of("/back", true);
    fake.nodes.get_mut("/back").expect("back").read.node.extents =
        Field::Present(crate::tree::Extents {
            x: 400,
            y: 300,
            width: 90,
            height: 40,
        });
    assert_eq!(resolved(&mut fake, &entry), "/back");
}

#[test]
fn a_candidate_whose_name_cannot_be_read_withholds_a_lone_match() {
    let mut fake = one_button();
    let mut unreadable = node(PUSH_BUTTON, "Back", 30, 30);
    unreadable.node.properties.name = Field::Failed("name unavailable".to_owned());
    fake.add(FRAME_PATH, "/other", unreadable);
    let entry = fake.entry_of("/back", true);
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(error.is_explicitly_retryable());
    assert!(is_incomplete(&error));
    assert_eq!(error.details.as_ref().expect("details")["complete"], false);
}

#[test]
fn an_identifier_basis_ref_resolves_on_its_unique_identifier() {
    let mut fake = Fake::new();
    for (path, identifier) in [("/one", "toolbar.one"), ("/two", "toolbar.two")] {
        let mut read = node(PUSH_BUTTON, identifier, 40, 20);
        read.node.properties.accessible_id = Field::Present(identifier.to_owned());
        fake.add(FRAME_PATH, path, read);
    }
    let mut entry = fake.entry_of("/one", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::NativeId));
    entry.scope.path.clear();
    assert_eq!(resolved(&mut fake, &entry), "/one");

    fake.nodes
        .get_mut("/one")
        .expect("one")
        .read
        .node
        .properties
        .accessible_id = Field::Present("toolbar.renamed".to_owned());
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
}

#[test]
fn two_nodes_that_share_the_identifier_are_ambiguous() {
    let mut fake = Fake::new();
    for path in ["/one", "/two"] {
        let mut read = node(PUSH_BUTTON, path, 40, 20);
        read.node.properties.accessible_id = Field::Present("row".to_owned());
        fake.add(FRAME_PATH, path, read);
    }
    let mut entry = fake.entry_of("/one", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::NativeId));
    entry.scope.path.clear();
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(candidate_count(&error), 2);
}

#[test]
fn an_identifier_basis_ref_with_no_stored_identifier_withholds_the_answer() {
    let mut fake = one_button();
    let mut entry = fake.entry_of("/back", true);
    entry.identity.basis = Some(IdentityBasis::NativeId);
    entry.identity.native_id = None;
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(is_incomplete(&error));
}

#[test]
fn a_description_basis_ref_resolves_on_its_description_alone() {
    let mut fake = Fake::new();
    for (path, description) in [("/first", "first row"), ("/second", "second row")] {
        let mut read = node(PUSH_BUTTON, "Save", 40, 20);
        read.node.properties.description = Field::Present(description.to_owned());
        fake.add(FRAME_PATH, path, read);
    }
    let mut entry = fake.entry_of("/first", true);
    assert_eq!(entry.identity.name.as_deref(), Some("Save"));
    entry.identity.description = Some("first row".to_owned());
    entry.identity.basis = Some(IdentityBasis::Description);
    entry.scope.path.clear();
    assert_eq!(resolved(&mut fake, &entry), "/first");

    fake.nodes
        .get_mut("/first")
        .expect("first")
        .read
        .node
        .properties
        .description = Field::Present("first row, updated".to_owned());
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
}

#[test]
fn a_description_that_cannot_be_read_withholds_the_answer() {
    let mut fake = Fake::new();
    let mut first = node(PUSH_BUTTON, "Save", 40, 20);
    first.node.properties.description = Field::Present("first row".to_owned());
    fake.add(FRAME_PATH, "/first", first);
    let mut unreadable = node(PUSH_BUTTON, "Save", 40, 20);
    unreadable.node.properties.description = Field::Failed("description unavailable".to_owned());
    fake.add(FRAME_PATH, "/second", unreadable);
    let mut entry = fake.entry_of("/first", true);
    entry.identity.description = Some("first row".to_owned());
    entry.identity.basis = Some(IdentityBasis::Description);
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(is_incomplete(&error));
}

#[test]
fn a_positional_basis_ref_keeps_its_stored_bounds_behavior() {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/panel", node(PANEL, "", 200, 100));
    let entry = fake.entry_of("/panel", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::Positional));
    assert_eq!(resolved(&mut fake, &entry), "/panel");

    fake.nodes
        .get_mut("/panel")
        .expect("panel")
        .read
        .node
        .extents = Field::Present(crate::tree::Extents {
        x: 0,
        y: 0,
        width: 200,
        height: 40,
    });
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
}
