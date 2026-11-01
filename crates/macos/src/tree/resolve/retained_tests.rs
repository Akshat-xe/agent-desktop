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
    let match_of = |saved: &RefEntry, token: Option<&str>| {
        candidate_match(&element, saved, &candidate(token, "Target"))
    };

    assert_eq!(
        match_of(&saved, Some("axtok1:00ff")).unwrap(),
        IdentityMatch::Match
    );
    assert_eq!(
        match_of(&saved, Some("axtok1:00fe")).unwrap(),
        IdentityMatch::NoMatch
    );
    assert_eq!(match_of(&saved, None).unwrap(), IdentityMatch::Unknown);
    assert_eq!(match_of(&entry(None), None).unwrap(), IdentityMatch::Match);
    assert_eq!(
        match_of(&entry(Some("host:1")), Some("axtok1:00ff"))
            .unwrap_err()
            .code,
        ErrorCode::StaleRef
    );
}
