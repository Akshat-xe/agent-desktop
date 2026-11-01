use agent_desktop_core::{DeliverySemantics, ErrorCode, IdentityBasis, RefEntry};

use super::*;
use crate::tree::Field;
use crate::tree::resolve::resolve_fake::{FRAME_PATH, Fake, deadline, node};
use crate::tree::vocabulary::role_numbers::{LABEL, LIST, PANEL, PUSH_BUTTON};

fn kind(error: &AdapterError) -> &str {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
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

/// A refusal that decided nothing: an incomplete read is never a match, and an
/// incomplete read dispatches nothing.
fn incomplete(error: &AdapterError) {
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(is_incomplete(error), "{error:?}");
    assert_eq!(error.details.as_ref().expect("details")["complete"], false);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

fn named_twins() -> Fake {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/back", node(PUSH_BUTTON, "Back", 30, 30));
    fake.add(FRAME_PATH, "/twin", node(PUSH_BUTTON, "Back", 40, 30));
    fake
}

#[test]
fn a_second_match_beyond_the_node_budget_never_resolves() {
    let mut fake = named_twins();
    let mut entry = fake.entry_of("/back", true);
    entry.scope.path.clear();
    assert_eq!(refusal(&mut fake, &entry).code, ErrorCode::AmbiguousTarget);

    fake.limits.max_nodes = 2;
    let error = refusal(&mut fake, &entry);
    incomplete(&error);
    assert_eq!(fake.reads.searches, 2);
}

#[test]
fn a_second_match_below_the_depth_limit_never_resolves() {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/back", node(PUSH_BUTTON, "Back", 30, 30));
    fake.add(FRAME_PATH, "/wrapper", node(PANEL, "wrapper", 300, 300));
    fake.add("/wrapper", "/inner", node(PUSH_BUTTON, "Back", 30, 30));
    let mut entry = fake.entry_of("/back", true);
    entry.scope.path.clear();
    assert_eq!(refusal(&mut fake, &entry).code, ErrorCode::AmbiguousTarget);

    fake.limits.max_raw_depth = 1;
    incomplete(&refusal(&mut fake, &entry));
}

#[test]
fn a_level_timeout_after_the_first_match_never_resolves() {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/back", node(PUSH_BUTTON, "Back", 30, 30));
    fake.add(FRAME_PATH, "/panel", node(PANEL, "panel", 300, 300));
    fake.add("/panel", "/slow", node(PUSH_BUTTON, "Slow", 30, 30));
    let mut entry = fake.entry_of("/back", true);
    entry.scope.path.clear();
    fake.faults.walk_errors.insert(
        "/slow".to_owned(),
        AdapterError::timeout("the level did not answer"),
    );
    incomplete(&refusal(&mut fake, &entry));
    assert_eq!(fake.reads.searches, 1);
}

#[test]
fn a_name_basis_ref_never_counts_or_matches_an_element_of_another_role() {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/save", node(PUSH_BUTTON, "Save", 60, 24));
    fake.add(FRAME_PATH, "/label", node(LABEL, "Save", 60, 24));
    let mut entry = fake.entry_of("/save", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::Name));
    entry.scope.path.clear();
    assert_eq!(resolved(&mut fake, &entry), "/save");

    let children = &mut fake.nodes.get_mut(FRAME_PATH).expect("frame").children;
    children.retain(|path| path != "/save");
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

fn identified_rows() -> Fake {
    let mut fake = Fake::new();
    for (path, identifier) in [("/one", "save.one"), ("/two", "save.two")] {
        let mut read = node(PUSH_BUTTON, "Save", 60, 24);
        read.node.properties.accessible_id = Field::Present(identifier.to_owned());
        fake.add(FRAME_PATH, path, read);
    }
    fake
}

#[test]
fn a_name_basis_ref_never_counts_or_matches_an_element_of_another_identifier_group() {
    let mut fake = identified_rows();
    let mut entry = fake.entry_of("/one", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::NativeId));
    entry.identity.basis = Some(IdentityBasis::Name);
    entry.scope.path.clear();
    assert_eq!(resolved(&mut fake, &entry), "/one");

    fake.nodes
        .get_mut("/one")
        .expect("one")
        .read
        .node
        .properties
        .accessible_id = Field::Present("save.renamed".to_owned());
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

fn replaced_object() -> (Fake, RefEntry) {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/first", node(LIST, "", 100, 40));
    let mut entry = fake.entry_of("/first", true);
    entry.identity.retained_object = Some(":1.2/first".to_owned());
    let children = &mut fake.nodes.get_mut(FRAME_PATH).expect("frame").children;
    children.remove(0);
    fake.add(FRAME_PATH, "/replacement", node(LIST, "", 100, 40));
    (fake, entry)
}

#[test]
fn an_old_retained_object_token_is_stale_whatever_rectangle_its_replacement_keeps() {
    let (mut fake, entry) = replaced_object();
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert_eq!(fake.reads.searches, 1);

    let mut live = entry.clone();
    live.identity.retained_object = Some(":1.2/replacement".to_owned());
    assert_eq!(resolved(&mut fake, &live), "/replacement");
}

#[test]
fn a_positional_ref_without_a_token_keeps_its_path_and_bounds_rule() {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/first", node(PANEL, "", 80, 20));
    fake.add(FRAME_PATH, "/second", node(PANEL, "", 140, 20));
    let entry = fake.entry_of("/first", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::Positional));
    assert!(entry.identity.retained_object.is_none());
    fake.nodes
        .get_mut(FRAME_PATH)
        .expect("frame")
        .children
        .swap(0, 1);
    assert_eq!(resolved(&mut fake, &entry), "/first");

    fake.nodes
        .get_mut("/first")
        .expect("first")
        .read
        .node
        .extents = Field::Present(crate::tree::Extents {
        x: 0,
        y: 0,
        width: 81,
        height: 20,
    });
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());

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
