use super::*;

struct RenamedLiveAdapter;

impl ObservationOps for RenamedLiveAdapter {
    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            identity: crate::adapter::live_identity("Renamed"),
            state: ElementState {
                role: "button".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(crate::Rect {
                x: 1.0,
                y: 1.0,
                width: 20.0,
                height: 20.0,
            }),
            available_actions: vec![capability::CLICK.into()],
        })
    }
}

impl ActionOps for RenamedLiveAdapter {}
impl InputOps for RenamedLiveAdapter {}
impl SystemOps for RenamedLiveAdapter {}

struct RenamedSameIdentifierAdapter;

impl ObservationOps for RenamedSameIdentifierAdapter {
    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            identity: crate::LiveIdentity {
                name: crate::LocatorField::Known("Delete".into()),
                description: crate::LocatorField::Absent,
                identifiers: crate::IdentifierEvidence::typed(
                    [crate::ElementIdentifier {
                        kind: crate::IdentifierKind::AxIdentifier,
                        value: "toolbar-action".into(),
                    }],
                    Some(0),
                    true,
                ),
            },
            state: ElementState {
                role: "button".into(),
                states: Vec::new(),
                value: None,
                enabled: Some(true),
                hidden: Some(false),
                offscreen: Some(false),
            },
            states_complete: true,
            bounds: Some(crate::Rect {
                x: 1.0,
                y: 1.0,
                width: 20.0,
                height: 20.0,
            }),
            available_actions: vec![capability::CLICK.into()],
        })
    }
}

impl ActionOps for RenamedSameIdentifierAdapter {}
impl InputOps for RenamedSameIdentifierAdapter {}
impl SystemOps for RenamedSameIdentifierAdapter {}

#[test]
fn a_ref_without_a_unique_identity_is_refused_before_the_live_read() {
    let mut ambiguous = entry();
    ambiguous.identity.basis = Some(crate::IdentityBasis::Ambiguous);
    let error = check_live(
        &ambiguous,
        &NativeHandle::null(),
        &UnsupportedLiveAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(
        error.details.as_ref().unwrap()["kind"],
        "identity_not_unique"
    );
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());

    let mut legacy = entry();
    legacy.identity.basis = None;
    let error = check_live(
        &legacy,
        &NativeHandle::null(),
        &UnsupportedLiveAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
}

#[test]
fn matching_bounds_cannot_confirm_an_identity_that_changed() {
    let named = entry();
    assert_eq!(named.identity.basis, Some(crate::IdentityBasis::Name));
    assert_eq!(named.identity.name.as_deref(), Some("OK"));

    let error = check_live(
        &named,
        &NativeHandle::null(),
        &RenamedLiveAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert!(error.message.contains("changed identity"));
    assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
}

#[test]
fn the_live_check_requires_the_recorded_name_behind_a_shared_identifier() {
    let mut named = entry();
    named.identity.basis = Some(crate::IdentityBasis::NativeId);
    named.identity.native_id = Some(crate::ElementIdentifier {
        kind: crate::IdentifierKind::AxIdentifier,
        value: "toolbar-action".into(),
    });
    named.identity.name = Some("Save".into());

    let error = check_live(
        &named,
        &NativeHandle::null(),
        &RenamedSameIdentifierAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(
        error.code,
        ErrorCode::StaleRef,
        "an identifier still present on a renamed control is not the captured element"
    );
    assert!(error.message.contains("changed identity"));
}
