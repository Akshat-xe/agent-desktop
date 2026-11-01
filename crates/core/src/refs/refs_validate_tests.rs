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

/// Writing a positional entry back must keep `basis` out of the JSON only when
/// no basis was recorded; a recorded one survives the round trip.
#[test]
fn a_recorded_basis_round_trips_while_a_legacy_entry_stays_absent() {
    let mut entry: RefEntry = serde_json::from_str(&legacy_ref_entry_json()).unwrap();
    assert_eq!(entry.identity.basis, None);
    assert!(validate_ref_entry(&entry).is_ok());
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
