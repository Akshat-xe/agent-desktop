use super::*;

#[test]
fn action_availability_makes_toggle_and_expand_applicable() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_entry({
        let mut entry = crate::adapter::minimal_ref_entry("cell", Some("Disclosure"));
        entry.capabilities.available_actions = vec!["Check".into(), "Expand".into()];
        entry.source.source_surface = crate::adapter::SnapshotSurface::Window;
        entry
    });
    let adapter = LiveStateAdapter {
        state: Mutex::new(None),
        bounds: Mutex::new(None),
        bounds_supported: false,
        state_supported: true,
    };

    for property in [IsProperty::Checked, IsProperty::Expanded] {
        let result = execute(
            IsArgs {
                ref_id: format!("@{}:e1", snapshot_id.clone()),
                property,
            },
            &adapter,
            &CommandContext::default(),
        )
        .unwrap();

        assert_eq!(result["applicable"], true);
    }
}
