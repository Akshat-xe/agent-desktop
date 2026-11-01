use super::*;

#[test]
fn ref_surface_fallback_uses_saved_number_without_bounds_hash() {
    let mut entry: RefEntry = serde_json::from_value(serde_json::json!({
        "pid": 1,
        "role": "button",
        "path": [],
        "states": [],
        "available_actions": [],
        "source_window_id": "w-42"
    }))
    .unwrap();
    entry.source.source_surface = SnapshotSurface::Sheet;
    for hash in [None, Some(123), Some(456)] {
        entry.source.source_window_bounds_hash = hash;
        assert_eq!(
            saved_surface_fallback(&entry, |number| Ok(Some(number))).unwrap(),
            Some(42)
        );
    }
    for id in [None, Some("invalid")] {
        entry.source.source_window_id = id.map(String::from);
        assert!(
            saved_surface_fallback::<i64>(&entry, |_| panic!("missing saved identity"))
                .unwrap()
                .is_none()
        );
    }
}
