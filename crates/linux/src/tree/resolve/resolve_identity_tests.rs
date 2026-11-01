use agent_desktop_core::{
    AdapterError, DeliverySemantics, ErrorCode, IdentityBasis, LocatorField, RefEntry,
};

use super::*;
use crate::tree::Field;
use crate::tree::live::test_support::{evidence, ref_entry};
use crate::tree::resolve::resolve;
use crate::tree::resolve::resolve_errors::is_incomplete;
use crate::tree::resolve::resolve_fake::{FRAME_PATH, Fake, deadline, node};
use crate::tree::vocabulary::role_numbers::{MENU_ITEM, PUSH_BUTTON};

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

fn set_name(fake: &mut Fake, path: &str, name: &str) {
    fake.nodes
        .get_mut(path)
        .expect("the node is in the tree")
        .read
        .node
        .properties
        .name = Field::Present(name.to_owned());
}

#[test]
fn an_identifier_basis_ref_never_resolves_a_button_whose_recorded_name_moved() {
    let mut fake = Fake::new();
    let mut save = node(PUSH_BUTTON, "Save", 40, 20);
    save.node.properties.accessible_id = Field::Present("toolbar.primary".to_owned());
    fake.add(FRAME_PATH, "/save", save);
    let mut entry = fake.entry_of("/save", true);
    assert_eq!(entry.identity.basis, Some(IdentityBasis::NativeId));
    assert_eq!(entry.identity.name.as_deref(), Some("Save"));
    entry.scope.path.clear();
    assert_eq!(resolved(&mut fake, &entry), "/save");

    set_name(&mut fake, "/save", "Delete");
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

#[test]
fn a_description_basis_ref_never_resolves_a_button_whose_recorded_name_moved() {
    let mut fake = Fake::new();
    let mut save = node(PUSH_BUTTON, "Save", 40, 20);
    save.node.properties.description = Field::Present("primary action".to_owned());
    fake.add(FRAME_PATH, "/save", save);
    let mut entry = fake.entry_of("/save", true);
    entry.identity.description = Some("primary action".to_owned());
    entry.identity.basis = Some(IdentityBasis::Description);
    entry.scope.path.clear();
    assert_eq!(resolved(&mut fake, &entry), "/save");

    set_name(&mut fake, "/save", "Delete");
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");
}

fn menu_item(description: &str) -> crate::tree::ObservedRead {
    let mut item = node(MENU_ITEM, "Open", 120, 24);
    item.node.properties.description = Field::Present(description.to_owned());
    item
}

#[test]
fn a_menu_ref_never_resolves_beside_another_item_whose_description_is_unread() {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/open", menu_item("open"));
    fake.add(FRAME_PATH, "/close", menu_item("close"));
    let mut entry = fake.entry_of("/open", true);
    entry.identity.description = Some("open".to_owned());
    entry.identity.basis = Some(IdentityBasis::Description);
    entry.identity.retained_object = Some(":1.2/open".to_owned());
    assert_eq!(resolved(&mut fake, &entry), "/open");

    fake.nodes
        .get_mut("/close")
        .expect("close")
        .read
        .node
        .properties
        .description = Field::Failed("description unavailable".to_owned());
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(is_incomplete(&error), "{error:?}");
    assert_eq!(error.details.as_ref().expect("details")["complete"], false);
    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
}

fn menu_item_entry() -> RefEntry {
    let mut entry = ref_entry("menuitem");
    entry.identity.name = Some("Open".to_owned());
    entry.identity.description = Some("open".to_owned());
    entry.identity.basis = Some(IdentityBasis::Description);
    entry.identity.retained_object = Some(":1.2/open".to_owned());
    entry
}

fn item_evidence(name: &str, description: LocatorField<String>, address: &str) -> LocatorEvidence {
    let mut live = evidence("menuitem", Some(name), None, None);
    live.description = description;
    live.identifiers = live
        .identifiers
        .clone()
        .with_retained_object(address.to_owned());
    live
}

#[test]
fn identity_admission_counts_full_matches_and_keeps_the_address_apart() {
    let entry = menu_item_entry();
    let open = || LocatorField::Known("open".to_owned());

    assert_eq!(
        identity_admission(&entry, &item_evidence("Open", open(), ":1.2/open")),
        Admission::Collect
    );
    assert_eq!(
        identity_admission(&entry, &item_evidence("Open", open(), ":1.2/close")),
        Admission::Partial
    );
    assert_eq!(
        identity_admission(
            &entry,
            &item_evidence("Open", LocatorField::Unknown, ":1.2/close")
        ),
        Admission::Unread
    );
    assert_eq!(
        identity_admission(
            &entry,
            &item_evidence("Open", LocatorField::Unknown, ":1.2/open")
        ),
        Admission::Unread
    );
    assert_eq!(
        identity_admission(
            &entry,
            &item_evidence("Open", LocatorField::Known("close".to_owned()), ":1.2/open")
        ),
        Admission::Reject
    );
    assert_eq!(
        identity_admission(&entry, &item_evidence("Delete", open(), ":1.2/open")),
        Admission::Reject
    );
}
