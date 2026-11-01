use super::*;

struct OmittedChildrenAdapter {
    omit: bool,
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
        crate::adapter::observed_tree(&root, raw)
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
    let resolve = |omit| {
        resolve_in_root(
            &OmittedChildrenAdapter { omit },
            &entry,
            &ScopeRoot::new(&root, NativeHandle::new(Vec::<usize>::new())),
            Deadline::standard().unwrap(),
        )
        .err()
    };
    assert!(resolve(false).is_none());
    assert_eq!(
        resolve(true).unwrap().details.unwrap()["kind"],
        "scoped_ref_incomplete"
    );
}

#[test]
fn a_survey_over_omitted_descendants_proves_no_basis() {
    use crate::locator::live_locator::test_support::{evidence, node, tree};
    let mut group = node(0, evidence("window", Some("Fixture")), vec![1], &[]);
    group.children_count = None;
    let mut deep = node(2, evidence("group", Some("Deep")), vec![], &[1]);
    deep.children_count = Some(3);
    let nodes = vec![
        group,
        node(1, evidence("button", Some("Save")), vec![], &[0]),
        deep,
    ];
    let mut observed = tree(nodes, vec![0], true);
    observed.nodes[0].children = vec![1, 2];
    let counts =
        super::super::materialize::survey_counts(&observed, &crate::LocatorQuery::default());
    let identity = crate::RefEntryIdentity {
        basis: None,
        retained_object: None,
        role: "button".into(),
        name: Some("Save".into()),
        value: None,
        description: None,
        native_id: None,
    };
    assert_eq!(counts.basis(&identity), IdentityBasis::Ambiguous);
    observed.nodes[2].children_count = None;
    let counts =
        super::super::materialize::survey_counts(&observed, &crate::LocatorQuery::default());
    assert_eq!(counts.basis(&identity), IdentityBasis::Name);
}
