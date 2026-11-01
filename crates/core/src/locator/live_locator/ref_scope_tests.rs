use super::*;
use crate::locator::live_locator::{
    IdentifierEvidence, LocatorEvidence, LocatorField, LocatorStats, ObservationSource,
    ObservedSubtree, test_support::evidence,
};

struct HydrationAdapter {
    survey: Vec<LocatorEvidence>,
    landing: LocatorEvidence,
}

impl crate::ObservationOps for HydrationAdapter {
    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::new(entry.identity.role.clone()))
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let ObservationRoot::Element { handle, .. } = root else {
            panic!("scope resolution must observe an element");
        };
        let leaf = |evidence: &LocatorEvidence| {
            ObservedSubtree::new(evidence.clone(), Vec::new(), true, None)
        };
        let subtree = if handle.downcast_ref::<String>().unwrap() == "group" {
            let children = if request.max_logical_depth > 0 {
                self.survey.iter().map(leaf).collect()
            } else {
                Vec::new()
            };
            ObservedSubtree::new(evidence("group", Some("Root")), children, true, None)
        } else {
            leaf(&self.landing)
        };
        ObservedTree::from_roots(
            vec![subtree],
            ObservationSource::from_root(&root, crate::SnapshotSurface::Window),
            LocatorStats::default(),
            true,
        )
    }
}

impl crate::ActionOps for HydrationAdapter {}
impl crate::InputOps for HydrationAdapter {}
impl crate::SystemOps for HydrationAdapter {}

fn resolve_under(
    root: &RefEntry,
    entry: &RefEntry,
    survey: Vec<LocatorEvidence>,
    landing: LocatorEvidence,
) -> Option<crate::ErrorCode> {
    let adapter = HydrationAdapter { survey, landing };
    resolve_in_root(
        &adapter,
        entry,
        root,
        &NativeHandle::new("group".to_string()),
        Deadline::standard().unwrap(),
    )
    .err()
    .map(|error| error.code)
}

fn resolve(
    entry: &RefEntry,
    survey: Vec<LocatorEvidence>,
    landing: LocatorEvidence,
) -> Option<crate::ErrorCode> {
    let root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    resolve_under(&root, entry, survey, landing)
}

fn resolve_alone(entry: &RefEntry, live: LocatorEvidence) -> Option<crate::ErrorCode> {
    resolve(entry, vec![live.clone()], live)
}

fn identified(mut evidence: LocatorEvidence, value: &str) -> LocatorEvidence {
    evidence.identifiers = IdentifierEvidence::typed([identifier(value)], Some(0), true);
    evidence
}

fn identifier(value: &str) -> crate::ElementIdentifier {
    crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: value.into(),
    }
}

fn live(name: Option<&str>, value: Option<&str>, description: Option<&str>) -> LocatorEvidence {
    let mut live = evidence("button", name);
    if let Some(value) = value {
        live.value = LocatorField::Known(value.into());
    }
    if let Some(description) = description {
        live.description = LocatorField::Known(description.into());
    }
    live
}

fn entry_of(basis: IdentityBasis, name: Option<&str>) -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", name);
    entry.identity.basis = Some(basis);
    entry
}

const STALE: Option<crate::ErrorCode> = Some(crate::ErrorCode::StaleRef);
const UNREAD: Option<crate::ErrorCode> = Some(crate::ErrorCode::Timeout);
const AMBIGUOUS: Option<crate::ErrorCode> = Some(crate::ErrorCode::AmbiguousTarget);

#[test]
fn scoped_count_uses_every_recorded_field_and_ignores_values_and_bounds() {
    let mut entry = entry_of(IdentityBasis::NativeId, Some("Save"));
    entry.identity.native_id = Some(identifier("shared"));
    entry.identity.description = Some("Writes the file".into());
    entry.identity.value = Some("old".into());
    entry.geometry.bounds_hash = Some(123);
    let matching = identified(
        live(Some("Save"), Some("new"), Some("Writes the file")),
        "shared",
    );
    let mut other = matching.clone();
    other.description = LocatorField::Known("Exports a copy".into());
    assert_eq!(
        resolve(&entry, vec![matching.clone(), other], matching.clone()),
        None
    );
    assert_eq!(
        resolve(&entry, vec![matching.clone(), matching.clone()], matching),
        AMBIGUOUS
    );

    let mut described = entry_of(IdentityBasis::Description, None);
    described.identity.description = Some("Billing".into());
    assert_eq!(
        resolve_alone(&described, live(Some("Billing"), None, None)),
        STALE
    );
    assert_eq!(
        resolve_alone(&described, live(None, Some("Billing"), None)),
        STALE
    );
    assert_eq!(
        resolve_alone(&described, live(None, None, Some("Billing"))),
        None
    );

    let named = entry_of(IdentityBasis::Name, Some("Address"));
    assert_eq!(
        resolve_alone(&named, live(None, Some("Address"), None)),
        STALE
    );
    assert_eq!(
        resolve_alone(&named, live(Some("Address"), None, None)),
        None
    );
    assert_eq!(
        resolve_alone(&named, live(Some("Address"), Some("Billing"), None)),
        None
    );

    let mut native = entry_of(IdentityBasis::NativeId, Some("Save"));
    native.identity.native_id = Some(identifier("toolbar-action"));
    assert_eq!(
        resolve_alone(
            &native,
            identified(live(Some("Delete"), None, None), "toolbar-action")
        ),
        STALE
    );
    assert_eq!(
        resolve_alone(
            &native,
            identified(live(None, Some("Save"), None), "toolbar-action")
        ),
        STALE
    );
    assert_eq!(
        resolve_alone(
            &native,
            identified(live(Some("Save"), None, None), "toolbar-action")
        ),
        None
    );
    let mut unread_name = identified(live(None, None, None), "toolbar-action");
    unread_name.name = LocatorField::Unknown;
    assert_eq!(resolve_alone(&native, unread_name), UNREAD);

    let mut described_name = entry_of(IdentityBasis::Name, Some("Address"));
    described_name.identity.description = Some("Billing".into());
    assert_eq!(
        resolve_alone(
            &described_name,
            live(Some("Address"), None, Some("Shipping"))
        ),
        STALE
    );
    assert_eq!(
        resolve_alone(&described_name, live(Some("Address"), None, None)),
        STALE
    );
    assert_eq!(
        resolve_alone(
            &described_name,
            live(Some("Address"), None, Some("Billing"))
        ),
        None
    );

    let mut shared = entry_of(IdentityBasis::Name, Some("Alpha"));
    shared.identity.native_id = Some(identifier("shared-row"));
    assert_eq!(
        resolve_alone(
            &shared,
            identified(live(Some("Birthday"), None, None), "shared-row")
        ),
        STALE
    );
    assert_eq!(
        resolve_alone(
            &shared,
            identified(live(Some("Alpha"), None, None), "shared-row")
        ),
        None
    );
    let mut first_instance = entry_of(IdentityBasis::Name, Some("Alpha"));
    first_instance.identity.native_id = Some(identifier("first-instance"));
    assert_eq!(
        resolve_alone(
            &first_instance,
            identified(live(Some("Alpha"), None, None), "second-instance")
        ),
        STALE
    );
    assert_eq!(
        resolve_alone(&first_instance, live(Some("Alpha"), None, None)),
        STALE
    );

    for value in ["On", "Off", ""] {
        assert_eq!(
            resolve_alone(&named_alpha(), live(Some("Alpha"), Some(value), None)),
            None,
            "value {value:?} must not decide a name match"
        );
    }
    assert_eq!(
        resolve_alone(&named_alpha(), live(Some("Beta"), Some("Alpha"), None)),
        STALE
    );
    let mut by_id = entry_of(IdentityBasis::NativeId, None);
    by_id.identity.native_id = Some(identifier("row-7"));
    for value in [Some("On"), Some("Off"), None] {
        assert_eq!(
            resolve_alone(&by_id, identified(live(None, value, None), "row-7")),
            None,
            "value {value:?} must not decide a native-id match"
        );
    }

    let mut entry = entry_of(IdentityBasis::NativeId, Some("Save"));
    entry.identity.native_id = Some(identifier("recorded"));
    let mut surveyed = live(Some("Save"), None, None);
    surveyed.identifiers = IdentifierEvidence::typed(
        [identifier("preferred"), identifier("recorded")],
        Some(0),
        true,
    );
    let mut landing = surveyed.clone();
    landing.identifiers = IdentifierEvidence::typed([identifier("preferred")], Some(0), true);
    assert_eq!(resolve(&entry, vec![surveyed.clone()], landing), STALE);
    assert_eq!(resolve(&entry, vec![surveyed.clone()], surveyed), None);
}

fn named_alpha() -> RefEntry {
    entry_of(IdentityBasis::Name, Some("Alpha"))
}

#[test]
fn positional_scoped_ref_keeps_its_relative_path_and_recorded_bounds() {
    let mut root = crate::adapter::minimal_ref_entry("group", Some("Root"));
    root.scope.path = smallvec::smallvec![2];
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.scope.path = smallvec::smallvec![2, 1];
    entry.identity.retained_object = Some(":1.2/first".into());
    let bounds = crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    entry.geometry.bounds_hash = bounds.bounds_hash();
    let mut matching = evidence("button", None);
    matching.ref_evidence.bounds = LocatorField::Known(bounds);
    matching.identifiers = IdentifierEvidence::absent().with_retained_object(":1.2/first".into());
    let filler = evidence("group", Some("Filler"));
    let under_root = |survey: Vec<LocatorEvidence>, landing: &LocatorEvidence| {
        resolve_under(&root, &entry, survey, landing.clone())
    };
    assert_eq!(
        under_root(vec![filler.clone(), matching.clone()], &matching),
        None
    );
    assert_eq!(under_root(vec![matching.clone()], &matching), STALE);

    let mut unbounded = matching.clone();
    unbounded.ref_evidence.bounds = LocatorField::Absent;
    assert_eq!(
        under_root(vec![filler.clone(), unbounded], &matching),
        STALE
    );

    let mut replaced = matching.clone();
    replaced.identifiers =
        IdentifierEvidence::absent().with_retained_object(":1.2/replacement".into());
    assert_eq!(
        under_root(vec![filler.clone(), replaced.clone()], &matching),
        STALE,
        "a replacement found by the survey is refused"
    );
    let mut untokened = matching.clone();
    untokened.identifiers = IdentifierEvidence::absent();
    assert_eq!(
        under_root(vec![filler.clone(), untokened], &matching),
        UNREAD
    );
    assert_eq!(
        under_root(vec![filler, matching.clone()], &replaced),
        STALE,
        "a replacement the anchor lands on after the survey is refused"
    );

    let mut windowed = crate::adapter::minimal_ref_entry("button", None);
    windowed.scope.path = smallvec::smallvec![0];
    windowed.geometry.bounds_space = Some(crate::BoundsSpace::Window);
    windowed.geometry.bounds_hash = bounds.bounds_hash();
    let mut surveyed = evidence("button", None);
    surveyed.ref_evidence.window_bounds = LocatorField::Known(bounds);
    let mut moved = surveyed.clone();
    moved.ref_evidence.window_bounds = LocatorField::Known(crate::Rect { x: 50.0, ..bounds });
    assert_eq!(resolve(&windowed, vec![surveyed.clone()], moved), STALE);
    assert_eq!(resolve(&windowed, vec![surveyed.clone()], surveyed), None);
}

#[test]
fn identity_basis_selection_checks_the_retained_object_of_the_counted_match() {
    let mut entry = entry_of(IdentityBasis::Name, Some("Save"));
    entry.identity.retained_object = Some(":1.2/first".into());
    let mut original = evidence("button", Some("Save"));
    original.identifiers = IdentifierEvidence::absent().with_retained_object(":1.2/first".into());
    let mut replaced = original.clone();
    replaced.identifiers = IdentifierEvidence::absent().with_retained_object(":1.2/other".into());
    assert_eq!(resolve_alone(&entry, original.clone()), None);
    assert_eq!(resolve_alone(&entry, replaced.clone()), STALE);
    assert_eq!(
        resolve(
            &entry,
            vec![replaced.clone(), replaced.clone()],
            replaced.clone()
        ),
        AMBIGUOUS
    );
    assert_eq!(
        resolve(&entry, vec![original.clone(), replaced], original),
        AMBIGUOUS
    );
}
