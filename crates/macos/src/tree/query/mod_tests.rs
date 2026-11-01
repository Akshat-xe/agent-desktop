use super::*;
use agent_desktop_core::{
    Deadline, IdentifierEvidence, IdentityBasis, IdentityPredicate, LocatorEvidence, LocatorField,
    LocatorMaterialization, LocatorQuery, LocatorRefEvidence, LocatorResolveRequest,
    LocatorSelection, ObservedSubtree, ProcessId, RefEntry, WindowInfo, evaluate_locator_tree,
    refs::identity_basis_gate::basis_gate,
};

fn subtree(role: &str, name: &str, children: Vec<ObservedSubtree>) -> ObservedSubtree {
    ObservedSubtree::new(
        LocatorEvidence {
            role: LocatorField::Known(role.into()),
            name: LocatorField::Known(name.into()),
            value: LocatorField::Absent,
            description: LocatorField::Absent,
            identifiers: IdentifierEvidence::absent(),
            states: LocatorField::Known(Vec::new()),
            ref_evidence: LocatorRefEvidence {
                bounds: LocatorField::Absent,
                window_bounds: LocatorField::Absent,
                available_actions: LocatorField::Known(Vec::new()),
                descriptors: Default::default(),
            },
        },
        children,
        true,
        None,
    )
}

fn capture_save(surface: SnapshotSurface, outside_save: bool, sheet_open: bool) -> RefEntry {
    let mut children = Vec::new();
    if sheet_open {
        children.push(subtree(
            "sheet",
            "Save document",
            vec![subtree("button", "Save", Vec::new())],
        ));
    }
    if outside_save {
        children.push(subtree("button", "Save", Vec::new()));
    }
    let root = match surface {
        SnapshotSurface::Window => subtree("window", "Editor", children),
        SnapshotSurface::Focused if sheet_open => children.remove(0),
        _ => panic!("the fixture needs a window or an open focused sheet"),
    };
    let observed = ObservedTree::from_roots(
        vec![root],
        ObservationSource::Window {
            window: WindowInfo {
                id: "w-11".into(),
                title: "Editor".into(),
                app: "Fixture".into(),
                pid: ProcessId::new(42),
                process_instance: Some("42:100".into()),
                bounds: None,
                state: Default::default(),
            },
            surface: SnapshotSurface::Focused,
        },
        Default::default(),
        true,
    )
    .unwrap();
    let mut resolution = evaluate_locator_tree(
        observed,
        &LocatorQuery {
            identity: IdentityPredicate {
                role: Some("button".into()),
                name: Some("Save".into()),
                ..Default::default()
            },
            ..Default::default()
        },
        &LocatorResolveRequest {
            selection: LocatorSelection::First,
            deadline: Deadline::after(1_000).unwrap(),
            max_raw_depth: 50,
            materialization: LocatorMaterialization::SelectedMatches,
            surface: None,
        },
    )
    .unwrap();
    assert!(resolution.meta.complete);
    resolution.matches.remove(0).into_entry()
}

#[test]
fn focused_capture_counts_the_save_button_outside_the_sheet() {
    let root_surface = admit_focused_owner("w-11", Some(11)).unwrap();
    let captured = capture_save(root_surface, true, true);
    let survivor = capture_save(SnapshotSurface::Window, true, false);

    assert_eq!(captured.identity.basis, Some(IdentityBasis::Ambiguous));
    assert_eq!(survivor.identity.basis, Some(IdentityBasis::Name));
    assert_eq!(captured.source, survivor.source);
    assert_eq!(captured.process, survivor.process);
    let error = basis_gate(&captured.identity).unwrap_err();
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(error.details.unwrap()["kind"], "identity_not_unique");
}

#[test]
fn focused_capture_keeps_a_unique_sheet_button_relative_to_the_window() {
    let root_surface = admit_focused_owner("w-11", Some(11)).unwrap();
    let captured = capture_save(root_surface, false, true);

    assert_eq!(captured.identity.basis, Some(IdentityBasis::Name));
    assert_eq!(captured.scope.path.as_slice(), &[0, 0]);
    assert!(basis_gate(&captured.identity).is_ok());
}

#[test]
fn focused_capture_walks_the_verified_owner_not_a_title_lookup_result() {
    let walked = verified_focused_owner("w-11", Some("window-a"), |owner| {
        assert_eq!(*owner, "window-a");
        Ok(Some(11))
    })
    .unwrap();
    assert_eq!(walked, "window-a");

    let refused = verified_focused_owner("w-11", Some("window-b"), |_| Ok(Some(12))).unwrap_err();
    assert_eq!(refused.code, ErrorCode::WindowNotFound);
    assert!(verified_focused_owner::<&str>("w-11", None, |_| Ok(None)).is_err());
}
