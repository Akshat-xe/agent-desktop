use super::*;

struct OmittedChildrenAdapter {
    omit: bool,
    sibling: Option<crate::locator::live_locator::LocatorEvidence>,
}

impl crate::ObservationOps for OmittedChildrenAdapter {
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
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let ObservationRoot::Element { handle, .. } = root else {
            panic!("scope resolution must observe an element");
        };
        let mut raw = crate::adapter::minimal_accessibility_node("group", "Root");
        if handle.downcast_ref::<Vec<usize>>().unwrap().is_empty() {
            let mut deep = crate::adapter::minimal_accessibility_node("group", "Deep");
            deep.children_count = self.omit.then_some(3);
            raw.children.push(deep);
            raw.children
                .push(crate::adapter::minimal_accessibility_node("button", "Save"));
        } else {
            raw = crate::adapter::minimal_accessibility_node("button", "Save");
        }
        let mut tree = crate::adapter::observed_tree(&root, raw)?;
        if let Some(sibling) = self.sibling.clone().filter(|_| tree.nodes.len() > 1) {
            let order = u32::try_from(tree.nodes.len()).unwrap();
            tree.nodes[0].children.push(order);
            tree.nodes
                .push(crate::locator::live_locator::test_support::node(
                    order,
                    sibling,
                    vec![],
                    &[2],
                ));
        }
        Ok(tree)
    }
}

impl crate::ActionOps for OmittedChildrenAdapter {}
impl crate::InputOps for OmittedChildrenAdapter {}
impl crate::SystemOps for OmittedChildrenAdapter {}

#[test]
fn a_full_count_with_omitted_descendants_never_selects_a_match() {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    let mut entry = crate::adapter::minimal_ref_entry("button", Some("Save"));
    entry.identity.basis = Some(IdentityBasis::Name);
    entry.scope.path = smallvec::smallvec![0];
    let resolve_with = |omit, sibling| {
        resolve_in_root(
            &OmittedChildrenAdapter { omit, sibling },
            &entry,
            &root,
            &NativeHandle::new(Vec::<usize>::new()),
            Deadline::standard().unwrap(),
        )
        .err()
    };
    let resolve = |omit| resolve_with(omit, None);
    assert!(resolve(false).is_none());
    assert_eq!(
        resolve(true).unwrap().details.unwrap()["kind"],
        "scoped_ref_incomplete"
    );

    use crate::locator::live_locator::{LocatorField, test_support::evidence};
    let mut unread_name = evidence("button", None);
    unread_name.name = LocatorField::Unknown;
    let code = |sibling| resolve_with(false, Some(sibling)).map(|error| error.code);
    assert_eq!(code(unread_name.clone()), Some(crate::ErrorCode::Timeout));
    let mut other_role = unread_name.clone();
    other_role.role = LocatorField::Known("group".into());
    assert_eq!(code(other_role), None);
    unread_name.role = LocatorField::Unknown;
    assert_eq!(code(unread_name), Some(crate::ErrorCode::Timeout));
}

struct AnchorRecorder {
    seen: std::sync::Mutex<Option<crate::RefEntryIdentity>>,
}

impl crate::ObservationOps for AnchorRecorder {
    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        *self.seen.lock().unwrap() = Some(entry.identity.clone());
        Ok(NativeHandle::new(entry.scope.path.to_vec()))
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let mut raw = crate::adapter::minimal_accessibility_node("textfield", "draft");
        raw.identity.value = Some("draft".into());
        raw.identity.description = Some("Address".into());
        crate::adapter::observed_tree(&root, raw)
    }
}

impl crate::ActionOps for AnchorRecorder {}
impl crate::InputOps for AnchorRecorder {}
impl crate::SystemOps for AnchorRecorder {}

#[test]
fn a_value_mirroring_name_is_not_sent_to_the_platform_as_stable_identity() {
    let mut entry = crate::adapter::minimal_ref_entry("textfield", Some("draft"));
    entry.identity.value = Some("draft".into());
    entry.identity.description = Some("Address".into());
    entry.identity.basis = Some(IdentityBasis::Description);
    let adapter = AnchorRecorder {
        seen: Default::default(),
    };
    resolve_anchor(&adapter, &entry, Deadline::standard().unwrap()).unwrap();
    let seen = adapter.seen.lock().unwrap().clone().unwrap();
    assert_eq!(seen.name, None);
    assert_eq!(seen.value, None);
    assert_eq!(seen.description.as_deref(), Some("Address"));
}
