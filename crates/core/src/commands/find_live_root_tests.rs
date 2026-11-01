use super::test_support_root::RootFindAdapter;
use super::*;
use crate::{
    refs::ref_resolve::resolve, refs::refs_store::RefStore, refs::refs_test_support::HomeGuard,
};

fn find(root: Option<&str>, role: Option<&str>, name: Option<&str>) -> FindArgs {
    FindArgs {
        app: None,
        window_id: Some("w-1".into()),
        root: root.map(Into::into),
        surface: crate::SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: role.map(Into::into),
            name: name.map(Into::into),
            description: None,
            native_id: None,
            value: None,
            text: None,
            exact: true,
        },
        states: Vec::new(),
        timeout_ms: None,
        selection: FindSelectionArgs {
            count: false,
            first: false,
            last: false,
            nth: None,
            limit: None,
        },
    }
}

fn panel_ref(context: &CommandContext) -> String {
    let response = execute(find(None, None, Some("Panel")), &RootFindAdapter, context).unwrap();
    response["matches"][0]["ref_id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn find_in_panel(role: Option<&str>, name: Option<&str>) -> Vec<String> {
    let context = CommandContext::default();
    let panel = panel_ref(&context);
    let response = execute(find(Some(&panel), role, name), &RootFindAdapter, &context).unwrap();
    response["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|found| found["ref_id"].as_str().unwrap().to_owned())
        .collect()
}

fn resolve_ref(ref_id: &str) -> Result<crate::NativeHandle, crate::AdapterError> {
    let context = CommandContext::default();
    let entry = crate::commands::helpers::load_ref_entry(ref_id, &context).unwrap();
    resolve(
        &RootFindAdapter,
        &entry,
        ref_id,
        &context,
        crate::Deadline::standard().unwrap(),
    )
}

fn resolved_path(ref_id: &str) -> Vec<usize> {
    let handle = resolve_ref(ref_id).expect("an unchanged UI must resolve the ref");
    handle.downcast_ref::<Vec<usize>>().unwrap().clone()
}

#[test]
fn a_unique_child_under_the_root_resolves_though_the_window_holds_a_twin() {
    let _guard = HomeGuard::new();
    let refs = find_in_panel(None, Some("Save"));
    assert_eq!(refs.len(), 1);
    assert_eq!(resolved_path(&refs[0]), [0, 0]);
}

#[test]
fn a_positional_match_under_the_root_resolves_with_its_root_chain() {
    let _guard = HomeGuard::new();
    let refs = find_in_panel(Some("textfield"), None);
    assert_eq!(refs.len(), 1);
    assert_eq!(resolved_path(&refs[0]), [0, 1]);
    let (snapshot, _) = crate::refs::ref_token::resolve_ref_target(&refs[0]).unwrap();
    let map = RefStore::new().unwrap().load_snapshot(&snapshot).unwrap();
    assert!(map.len() > 1, "the root must travel with the matches");
}

#[test]
fn a_twin_inside_the_root_is_ambiguous() {
    let _guard = HomeGuard::new();
    let refs = find_in_panel(None, Some("Dup"));
    assert_eq!(refs.len(), 2);
    for ref_id in refs {
        let error = resolve_ref(&ref_id).err().expect("a twin must not resolve");
        assert_eq!(error.code, crate::ErrorCode::AmbiguousTarget);
    }
}
