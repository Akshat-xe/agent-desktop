use super::*;
use crate::locator::live_locator::{IdentifierEvidence, LocatorField, test_support::*};

struct HydrationAdapter {
    surveyed: crate::AccessibilityNode,
    hydrated: crate::AccessibilityNode,
}

impl crate::ObservationOps for HydrationAdapter {
    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new(entry.scope.path.to_vec()))
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let ObservationRoot::Element { handle, .. } = root else {
            panic!("scope resolution must observe an element");
        };
        let target = !handle.downcast_ref::<Vec<usize>>().unwrap().is_empty();
        let mut raw = if target {
            self.hydrated.clone()
        } else {
            crate::adapter::minimal_accessibility_node("group", "Root")
        };
        if !target && request.max_logical_depth > 0 {
            raw.children.push(self.surveyed.clone());
        }
        let mut tree = crate::adapter::observed_tree(&root, raw)?;
        let index = if target {
            Some(0)
        } else if tree.nodes.len() > 1 {
            Some(1)
        } else {
            None
        };
        if let Some(index) = index {
            let original = if target {
                &self.hydrated
            } else {
                &self.surveyed
            };
            if !original.identity.native_ids.is_empty() {
                tree.nodes[index].evidence.identifiers =
                    IdentifierEvidence::typed(original.identity.native_ids.clone(), Some(0), true);
            }
            if let Some(token) = original.identity.retained_object.clone() {
                let identifiers = std::mem::take(&mut tree.nodes[index].evidence.identifiers);
                tree.nodes[index].evidence.identifiers = identifiers.with_retained_object(token);
            }
            tree.nodes[index].evidence.ref_evidence.window_bounds = original
                .presentation
                .ref_bounds
                .map(LocatorField::Known)
                .unwrap_or(LocatorField::Absent);
        }
        Ok(tree)
    }
}

impl crate::ActionOps for HydrationAdapter {}
impl crate::InputOps for HydrationAdapter {}
impl crate::SystemOps for HydrationAdapter {}

#[test]
fn hydration_keeps_the_recorded_identifier_when_the_live_preferred_identifier_changes() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Save"));
    let recorded = crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: "recorded".into(),
    };
    let preferred = crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: "preferred".into(),
    };
    entry.identity.basis = Some(IdentityBasis::NativeId);
    entry.identity.native_id = Some(recorded.clone());
    entry.scope.path = smallvec::smallvec![0];
    let mut surveyed = crate::adapter::minimal_accessibility_node("button", "Save");
    surveyed.identity.native_id = Some(preferred.clone());
    surveyed.identity.native_ids = vec![preferred, recorded];
    let mut hydrated = surveyed.clone();
    hydrated.identity.native_ids.truncate(1);
    let mut adapter = HydrationAdapter { surveyed, hydrated };
    let handle = NativeHandle::new(Vec::<usize>::new());
    let error = resolve_in_root(
        &adapter,
        &entry,
        &root,
        &handle,
        Deadline::standard().unwrap(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, crate::ErrorCode::StaleRef);
    adapter.hydrated = adapter.surveyed.clone();
    assert!(
        resolve_in_root(
            &adapter,
            &entry,
            &root,
            &handle,
            Deadline::standard().unwrap()
        )
        .is_ok()
    );
}

#[test]
fn positional_hydration_rechecks_window_bounds_after_the_anchor_lands() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.scope.path = smallvec::smallvec![0];
    entry.geometry.bounds_space = Some(crate::BoundsSpace::Window);
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    entry.geometry.bounds_hash = bounds.bounds_hash();
    let mut surveyed = crate::adapter::minimal_accessibility_node("button", "");
    surveyed.presentation.ref_bounds = Some(bounds);
    let mut hydrated = surveyed.clone();
    hydrated.presentation.ref_bounds = Some(crate::Rect { x: 50.0, ..bounds });
    let mut adapter = HydrationAdapter { surveyed, hydrated };
    let handle = NativeHandle::new(Vec::<usize>::new());
    let error = resolve_in_root(
        &adapter,
        &entry,
        &root,
        &handle,
        Deadline::standard().unwrap(),
    )
    .err()
    .unwrap();
    assert_eq!(error.code, crate::ErrorCode::StaleRef);
    adapter.hydrated = adapter.surveyed.clone();
    assert!(
        resolve_in_root(
            &adapter,
            &entry,
            &root,
            &handle,
            Deadline::standard().unwrap()
        )
        .is_ok()
    );
}

#[test]
fn scoped_count_uses_every_recorded_field_and_ignores_values_and_bounds() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Save"));
    entry.identity.basis = Some(IdentityBasis::NativeId);
    entry.identity.native_id = Some(crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: "shared".into(),
    });
    entry.identity.description = Some("Writes the file".into());
    entry.identity.value = Some("old".into());
    entry.geometry.bounds_hash = Some(123);
    let mut matching = evidence("button", Some("Save"));
    matching.description = LocatorField::Known("Writes the file".into());
    matching.value = LocatorField::Known("new".into());
    matching.identifiers =
        IdentifierEvidence::typed(entry.identity.native_id.clone(), Some(0), true);
    let mut other = matching.clone();
    other.description = LocatorField::Known("Exports a copy".into());
    let mut tree = tree(
        vec![
            node(0, matching.clone(), vec![], &[0]),
            node(1, other, vec![], &[1]),
        ],
        vec![0, 1],
        true,
    );
    assert_eq!(select(&tree, &entry, &root).unwrap().path.as_slice(), [0]);
    tree.nodes[1].evidence = matching;
    assert_eq!(
        select(&tree, &entry, &root).unwrap_err().code,
        crate::ErrorCode::AmbiguousTarget
    );
}

#[test]
fn unread_same_role_identity_prevents_a_unique_scoped_match() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    let entry = crate::adapter::minimal_ref_entry("button", Some("Save"));
    let mut unknown = evidence("button", None);
    unknown.name = LocatorField::Unknown;
    let mut tree = tree(
        vec![
            node(0, evidence("button", Some("Save")), vec![], &[0]),
            node(1, unknown, vec![], &[1]),
        ],
        vec![0, 1],
        true,
    );
    assert_eq!(
        select(&tree, &entry, &root).unwrap_err().code,
        crate::ErrorCode::Timeout
    );
    tree.nodes[1].evidence.role = LocatorField::Known("group".into());
    assert_eq!(select(&tree, &entry, &root).unwrap().path.as_slice(), [0]);
    tree.nodes[1].evidence.role = LocatorField::Unknown;
    assert_eq!(
        select(&tree, &entry, &root).unwrap_err().code,
        crate::ErrorCode::Timeout
    );
}

#[test]
fn positional_scoped_ref_keeps_its_relative_path_and_recorded_bounds() {
    let mut root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    root.scope.path = smallvec::smallvec![2];
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.scope.path = smallvec::smallvec![2, 1];
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    entry.geometry.bounds_hash = bounds.bounds_hash();
    let mut matching = evidence("button", None);
    matching.ref_evidence.bounds = LocatorField::Known(bounds);
    let mut tree = tree(vec![node(0, matching, vec![], &[1])], vec![0], true);
    assert!(select(&tree, &entry, &root).is_ok());
    tree.nodes[0].path = smallvec::smallvec![0];
    assert_eq!(
        select(&tree, &entry, &root).unwrap_err().code,
        crate::ErrorCode::StaleRef
    );
    tree.nodes[0].path = smallvec::smallvec![1];
    tree.nodes[0].evidence.ref_evidence.bounds = LocatorField::Absent;
    assert_eq!(
        select(&tree, &entry, &root).unwrap_err().code,
        crate::ErrorCode::StaleRef
    );
}

fn retained_field(
    token: Option<&str>,
) -> (RefEntry, crate::locator::live_locator::LocatorEvidence) {
    let mut entry = crate::adapter::minimal_ref_entry("textfield", None);
    entry.scope.path = smallvec::smallvec![1];
    entry.identity.retained_object = Some(":1.2/first".into());
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    entry.geometry.bounds_hash = bounds.bounds_hash();
    let mut live = evidence("textfield", None);
    live.ref_evidence.bounds = LocatorField::Known(bounds);
    if let Some(token) = token {
        live.identifiers = IdentifierEvidence::absent().with_retained_object(token.into());
    }
    (entry, live)
}

#[test]
fn positional_selection_refuses_a_replacement_with_another_retained_token() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    for (token, expected) in [
        (Some(":1.2/first"), None),
        (Some(":1.2/replacement"), Some(crate::ErrorCode::StaleRef)),
        (None, Some(crate::ErrorCode::Timeout)),
    ] {
        let (entry, live) = retained_field(token);
        let tree = tree(vec![node(0, live, vec![], &[1])], vec![0], true);
        assert_eq!(select(&tree, &entry, &root).err().map(|e| e.code), expected);
    }
}

#[test]
fn identity_basis_selection_checks_the_retained_object_of_the_counted_match() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    let (mut entry, mut live) = retained_field(Some(":1.2/first"));
    entry.identity.name = Some("Save".into());
    entry.identity.basis = Some(IdentityBasis::Name);
    live.name = LocatorField::Known("Save".into());
    let mut replaced = live.clone();
    replaced.identifiers = IdentifierEvidence::absent().with_retained_object(":1.2/other".into());
    let mut tree = tree(vec![node(0, live.clone(), vec![], &[1])], vec![0], true);
    assert_eq!(select(&tree, &entry, &root).unwrap().path.as_slice(), [1]);
    tree.nodes[0].evidence = replaced.clone();
    assert_eq!(
        select(&tree, &entry, &root).unwrap_err().code,
        crate::ErrorCode::StaleRef
    );
    tree.nodes.push(node(1, replaced, vec![], &[2]));
    tree.roots.push(1);
    assert_eq!(
        select(&tree, &entry, &root).unwrap_err().code,
        crate::ErrorCode::AmbiguousTarget
    );
}

#[test]
fn hydration_refuses_a_landing_with_another_retained_token() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.scope.path = smallvec::smallvec![0];
    entry.identity.retained_object = Some(":1.2/first".into());
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    entry.geometry.bounds_hash = bounds.bounds_hash();
    entry.geometry.bounds_space = Some(crate::BoundsSpace::Window);
    let mut surveyed = crate::adapter::minimal_accessibility_node("button", "");
    surveyed.presentation.ref_bounds = Some(bounds);
    surveyed.identity.retained_object = Some(":1.2/first".into());
    let mut hydrated = surveyed.clone();
    hydrated.identity.retained_object = Some(":1.2/replacement".into());
    let mut adapter = HydrationAdapter { surveyed, hydrated };
    let handle = NativeHandle::new(Vec::<usize>::new());
    let deadline = Deadline::standard().unwrap();
    let error = resolve_in_root(&adapter, &entry, &root, &handle, deadline)
        .err()
        .unwrap();
    assert_eq!(error.code, crate::ErrorCode::StaleRef);
    adapter.hydrated = adapter.surveyed.clone();
    assert!(resolve_in_root(&adapter, &entry, &root, &handle, deadline).is_ok());
}
