use super::*;

#[test]
fn native_id_ancestor_covers_unnamed_boundaries() {
    let mut split_view = node("group");
    split_view.identity.native_id = Some(crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: "main, SidebarNavigationSplitView".into(),
    });
    split_view.children = (0..3)
        .map(|_| {
            let mut boundary = node("group");
            boundary.children_count = Some(1);
            boundary
        })
        .collect();
    let mut root = node("window");
    root.children = vec![split_view];

    let mut refmap = RefMap::new();
    let result = ref_alloc::allocate_refs(root, &mut refmap, &run_config(false, false)).unwrap();

    let split_view = &result.children[0];
    assert!(split_view.ref_id.is_some());
    assert!(
        split_view
            .children
            .iter()
            .all(|boundary| boundary.ref_id.is_none())
    );
    let entry = refmap.get(split_view.ref_id.as_deref().unwrap()).unwrap();
    assert_eq!(
        entry
            .identity
            .native_id
            .as_ref()
            .map(|identifier| identifier.value.as_str()),
        Some("main, SidebarNavigationSplitView")
    );
}

#[test]
fn web_boundary_with_only_ubiquitous_affordances_gets_anchor_ref() {
    let mut boundary = node("group");
    boundary.identity.name = Some("Task list".into());
    boundary.presentation.available_actions = vec!["RightClick".into(), "ScrollTo".into()];
    boundary.children_count = Some(40);
    let mut root = node("window");
    root.children = vec![boundary];

    let mut refmap = RefMap::new();
    let result = ref_alloc::allocate_refs(root, &mut refmap, &run_config(false, false)).unwrap();

    let boundary = &result.children[0];
    let entry = refmap.get(boundary.ref_id.as_deref().unwrap()).unwrap();
    assert!(entry.capabilities.available_actions.is_empty());
    assert_eq!(refmap.len(), 1);
}

#[test]
fn unlabeled_bounded_boundary_gets_drill_ref() {
    let mut boundary = node("group");
    boundary.children_count = Some(5);
    boundary.presentation.bounds = Some(crate::Rect {
        x: 10.0,
        y: 20.0,
        width: 300.0,
        height: 400.0,
    });
    let mut root = node("window");
    root.children = vec![boundary];

    let mut refmap = RefMap::new();
    let result = ref_alloc::allocate_refs(root, &mut refmap, &run_config(false, false)).unwrap();

    let boundary = &result.children[0];
    assert!(boundary.ref_id.is_some());
    assert!(boundary.presentation.bounds.is_none());
    let entry = refmap.get(boundary.ref_id.as_deref().unwrap()).unwrap();
    assert!(
        entry.geometry.bounds.is_some(),
        "geometry is this anchor's only evidence, and the promotion that resolves it needs the rect, not just the hash"
    );
    assert!(entry.geometry.bounds_hash.is_some());
}

/// A uniquely named boundary records a name basis from the observed skeleton nodes; a twin
/// among them makes it ambiguous, and an ambiguous entry keeps the rect that could locate it.
#[test]
fn a_named_boundary_with_a_skeleton_twin_is_ambiguous_and_keeps_its_rect() {
    let mut boundary = node("group");
    boundary.identity.name = Some("Sidebar".into());
    boundary.children_count = Some(5);
    boundary.presentation.bounds = Some(crate::Rect {
        x: 10.0,
        y: 20.0,
        width: 300.0,
        height: 400.0,
    });
    let unique = {
        let mut root = node("window");
        root.children = vec![boundary.clone()];
        let mut refmap = RefMap::new();
        let result =
            ref_alloc::allocate_refs(root, &mut refmap, &run_config(false, false)).unwrap();
        let id = result.children[0].ref_id.clone().unwrap();
        refmap.get(&id).unwrap().identity.basis
    };
    assert_eq!(unique, Some(crate::IdentityBasis::Name));
    let mut root = node("window");
    root.children = vec![boundary.clone(), boundary];

    let mut refmap = RefMap::new();
    let result = ref_alloc::allocate_refs(root, &mut refmap, &run_config(false, false)).unwrap();

    let boundary = &result.children[0];
    let entry = refmap.get(boundary.ref_id.as_deref().unwrap()).unwrap();
    assert_eq!(entry.identity.basis, Some(crate::IdentityBasis::Ambiguous));
    assert!(!crate::refs::ref_identity::has_meaningful_identity(entry));
    assert!(
        entry.geometry.bounds.is_some(),
        "an entry whose identity is not proven keeps the geometry that could still locate it"
    );

    let mut button = node("button");
    button.identity.name = Some("Save".into());
    button.presentation.bounds = Some(crate::Rect {
        x: 0.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    });
    let mut root = node("window");
    root.children = vec![button];

    let mut refmap = RefMap::new();
    let result = ref_alloc::allocate_refs(root, &mut refmap, &run_config(false, false)).unwrap();

    let entry = refmap
        .get(result.children[0].ref_id.as_deref().unwrap())
        .unwrap();
    assert_eq!(entry.identity.basis, Some(crate::IdentityBasis::Name));
    assert!(crate::refs::ref_identity::has_meaningful_identity(entry));
    assert!(entry.geometry.bounds.is_none());
}

#[test]
fn deepest_resolvable_anchor_wins() {
    let mut boundary = node("group");
    boundary.children_count = Some(5);
    boundary.presentation.bounds = Some(crate::Rect {
        x: 10.0,
        y: 20.0,
        width: 300.0,
        height: 400.0,
    });
    let mut split_view = node("group");
    split_view.identity.native_id = Some(crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: "split-view".into(),
    });
    split_view.children = vec![boundary];
    let mut root = node("window");
    root.children = vec![split_view];

    let mut refmap = RefMap::new();
    let result = ref_alloc::allocate_refs(root, &mut refmap, &run_config(false, false)).unwrap();

    assert!(result.children[0].ref_id.is_none());
    assert!(result.children[0].children[0].ref_id.is_some());
    assert_eq!(refmap.len(), 1);
}
