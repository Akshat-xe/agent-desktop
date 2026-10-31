use super::*;
use crate::tree::resolve::resolve_fake::{FRAME_PATH, Fake, deadline, node};
use crate::tree::vocabulary::role_numbers::{PANEL, PUSH_BUTTON};

fn row_with_label() -> Fake {
    let mut fake = Fake::new();
    fake.add(FRAME_PATH, "/row", node(PUSH_BUTTON, "alpha", 300, 30));
    fake.add("/row", "/label", node(PUSH_BUTTON, "alpha", 200, 20));
    fake
}

#[test]
fn a_row_and_the_label_inside_it_are_two_matches_of_one_name() {
    let mut fake = row_with_label();
    for path in ["/row", "/label"] {
        let mut entry = fake.entry_of(path, false);
        entry.geometry.bounds = None;
        let error = resolve(&mut fake, &entry, deadline()).expect_err("two nodes carry the name");
        assert_eq!(error.code, ErrorCode::AmbiguousTarget);
        assert_eq!(error.details.expect("details")["candidate_count"], 2);
    }
}

#[test]
fn separate_rows_with_the_same_name_stay_ambiguous() {
    let mut fake = row_with_label();
    fake.add(FRAME_PATH, "/other", node(PANEL, "list", 300, 30));
    fake.add("/other", "/twin", node(PUSH_BUTTON, "alpha", 200, 20));
    let mut entry = fake.entry_of("/label", false);
    entry.geometry.bounds = None;
    let error = resolve(&mut fake, &entry, deadline()).expect_err("twins are refused");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}
