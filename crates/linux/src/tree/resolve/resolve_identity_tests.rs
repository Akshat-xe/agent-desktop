use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode, IdentityBasis, RefEntry};

use crate::tree::Field;
use crate::tree::resolve::resolve;
use crate::tree::resolve::resolve_errors::is_incomplete;
use crate::tree::resolve::resolve_fake::{FRAME_PATH, Fake, deadline, node};
use crate::tree::vocabulary::role_numbers::MENU_ITEM;

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

fn set_description_field(fake: &mut Fake, path: &str, field: Field<String>) {
    fake.nodes
        .get_mut(path)
        .expect("the item is in the menu")
        .read
        .node
        .properties
        .description = field;
}

fn set_description(fake: &mut Fake, path: &str, description: &str) {
    set_description_field(fake, path, Field::Present(description.to_owned()));
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

    set_description(&mut fake, "/close", "close");
    assert_eq!(resolved(&mut fake, &entry), "/open");
    set_description_field(&mut fake, "/open", Field::Failed("unread".to_owned()));
    let error = refusal(&mut fake, &entry);
    assert!(is_incomplete(&error), "{error:?}");

    set_description(&mut fake, "/open", "close");
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");

    set_description(&mut fake, "/open", "open");
    set_name(&mut fake, "/open", "Delete");
    let error = refusal(&mut fake, &entry);
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(kind(&error), "resolve_no_candidate");

    set_name(&mut fake, "/open", "Open");
    assert_eq!(resolved(&mut fake, &entry), "/open");
}
