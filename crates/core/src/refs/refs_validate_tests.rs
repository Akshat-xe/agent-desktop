use super::*;

fn legacy_ref_entry_json() -> String {
    serde_json::json!({
        "pid": 1,
        "process_instance": "legacy-instance",
        "role": "button",
        "name": "Save",
        "bounds_hash": null,
        "source_app": null,
        "available_actions": []
    })
    .to_string()
}

/// A refmap written before identity bases existed carries no `basis` field.
/// It must stay loadable, with `None` kept as `None`, so resolution can refuse
/// it as stale instead of guessing that its position was its identity.
#[test]
fn a_ref_entry_without_a_basis_loads_with_basis_none() {
    let entry: RefEntry = serde_json::from_str(&legacy_ref_entry_json()).unwrap();

    assert_eq!(entry.identity.basis, None);
    assert!(validate_ref_entry(&entry).is_ok());
}

/// Writing a positional entry back must keep `basis` out of the JSON only when
/// no basis was recorded; a recorded one survives the round trip.
#[test]
fn a_recorded_basis_round_trips_while_a_legacy_entry_stays_absent() {
    let mut entry: RefEntry = serde_json::from_str(&legacy_ref_entry_json()).unwrap();
    let legacy_json = serde_json::to_string(&entry).unwrap();
    assert!(!legacy_json.contains("basis"), "json={legacy_json}");

    entry.identity.basis = Some(crate::IdentityBasis::Positional);
    let json = serde_json::to_string(&entry).unwrap();
    assert!(json.contains("\"basis\":\"positional\""), "json={json}");
    let restored: RefEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(
        restored.identity.basis,
        Some(crate::IdentityBasis::Positional)
    );
}
