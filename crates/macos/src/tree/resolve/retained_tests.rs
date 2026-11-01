use super::*;
use agent_desktop_core::{LocatorField, LocatorRefEvidence, SnapshotSurface};

fn entry(token: Option<&str>) -> RefEntry {
    RefEntry {
        process: agent_desktop_core::RefProcess {
            pid: agent_desktop_core::ProcessId::new(1),
            process_instance: None,
        },
        identity: agent_desktop_core::RefEntryIdentity {
            retained_object: token.map(str::to_owned),
            role: "button".into(),
            name: Some("Target".into()),
            value: None,
            description: None,
            native_id: None,
            unique_at_capture: false,
        },
        geometry: agent_desktop_core::RefGeometry {
            bounds_space: None,
            bounds: None,
            bounds_hash: None,
        },
        capabilities: agent_desktop_core::RefCapabilities {
            states: Vec::new(),
            available_actions: Vec::new(),
        },
        source: agent_desktop_core::RefSource {
            source_app: None,
            source_window_id: None,
            source_window_title: None,
            source_window_bounds_hash: None,
            source_surface: SnapshotSurface::Window,
        },
        scope: agent_desktop_core::RefScope {
            root_ref: None,
            path_is_absolute: false,
            path: Default::default(),
        },
    }
}

fn candidate(token: Option<&str>, name: &str) -> LocatorEvidence {
    LocatorEvidence {
        role: LocatorField::Known("button".into()),
        name: LocatorField::Known(name.into()),
        description: LocatorField::Absent,
        value: LocatorField::Absent,
        identifiers: token.map_or_else(IdentifierEvidence::absent, |token| {
            IdentifierEvidence::absent().with_retained_object(token.into())
        }),
        states: LocatorField::Absent,
        ref_evidence: LocatorRefEvidence {
            window_bounds: LocatorField::Absent,
            bounds: LocatorField::Absent,
            available_actions: LocatorField::Absent,
            descriptors: Default::default(),
        },
    }
}

#[test]
fn a_candidate_matches_only_its_own_token() {
    let element = AXElement(std::ptr::null_mut());
    let saved = entry(Some("axtok1:00ff"));
    assert_eq!(
        candidate_match(&element, &saved, &candidate(Some("axtok1:00ff"), "Target")).unwrap(),
        IdentityMatch::Match
    );
    assert_eq!(
        candidate_match(&element, &saved, &candidate(Some("axtok1:00fe"), "Target")).unwrap(),
        IdentityMatch::NoMatch
    );
    assert_eq!(
        candidate_match(&element, &saved, &candidate(None, "Target")).unwrap(),
        IdentityMatch::Unknown
    );
}

#[test]
fn tokenless_entries_keep_text_identity() {
    let element = AXElement(std::ptr::null_mut());
    let saved = entry(None);
    assert_eq!(
        candidate_match(&element, &saved, &candidate(None, "Target")).unwrap(),
        IdentityMatch::Match
    );
    assert_eq!(
        candidate_match(&element, &saved, &candidate(None, "Other")).unwrap(),
        IdentityMatch::NoMatch
    );
}

#[test]
fn a_legacy_token_fails_closed() {
    let element = AXElement(std::ptr::null_mut());
    let error = candidate_match(
        &element,
        &entry(Some("host:1")),
        &candidate(Some("axtok1:00ff"), "Target"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn token_text_round_trips_through_lowercase_hex() {
    let bytes = [0x00, 0x1f, 0xab, 0xff];
    let hex = encode_hex(&bytes);
    assert_eq!(hex, "001fabff");
    assert_eq!(decode_hex(&hex).as_deref(), Some(&bytes[..]));
    assert_eq!(decode_hex("AABB").as_deref(), Some(&[0xaa, 0xbb][..]));
}

#[test]
fn malformed_hex_is_refused() {
    assert!(decode_hex("").is_none());
    assert!(decode_hex("0").is_none());
    assert!(decode_hex("zz").is_none());
    assert!(decode_hex("00 1f").is_none());
}

#[test]
fn only_remote_tokens_are_validateable() {
    assert!(validate(None).is_ok());
    assert!(validate(Some("axtok1:00ff")).is_ok());
    let error = validate(Some("host:1")).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::not_delivered()
    );
}

#[test]
fn a_legacy_session_token_is_not_a_remote_token() {
    assert!(matches!(
        resolve_token("host:1").unwrap(),
        TokenLookup::NotAToken
    ));
    assert!(matches!(resolve_token("").unwrap(), TokenLookup::NotAToken));
}

#[test]
fn a_null_element_yields_no_token() {
    let error = capture(&AXElement(std::ptr::null_mut())).unwrap_err();
    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn a_live_lookup_carries_the_element_it_resolved() {
    let raw = unsafe { accessibility_sys::AXUIElementCreateApplication(1) };
    let live = TokenLookup::Live(AXElement(raw));
    let TokenLookup::Live(element) = live else {
        return;
    };
    assert!(!element.0.is_null());
}
