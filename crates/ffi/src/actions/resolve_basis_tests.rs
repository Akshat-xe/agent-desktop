use super::*;

#[test]
fn decoded_entries_record_the_basis_their_own_fields_prove() {
    let role = CString::new("button").unwrap();
    let native = CString::new("row-7").unwrap();
    let mut entry = test_ref_entry();
    entry.identity.role = role.as_ptr();
    entry.identity.native_id = native.as_ptr();
    let exact = AdExactRefEntry {
        version: crate::types::exact_ref_entry::AD_EXACT_REF_ENTRY_VERSION,
        size: crate::types::exact_ref_entry::AD_EXACT_REF_ENTRY_SIZE as u32,
        entry,
        process_instance: c"42:100".as_ptr(),
        identifier_kind: AdIdentifierKind::AxIdentifier as i32,
    };
    let decoded = unsafe { core_ref_entry_from_exact(&exact) }.unwrap();
    assert_eq!(decoded.identity.basis, Some(IdentityBasis::NativeId));

    let mut named = test_ref_entry();
    named.identity.role = role.as_ptr();
    named.identity.name = c"target".as_ptr();
    let decoded = decode(named).unwrap();
    assert_eq!(decoded.identity.basis, Some(IdentityBasis::Name));

    let mut described = test_ref_entry();
    described.identity.role = role.as_ptr();
    described.identity.description = c"Opens the drawer".as_ptr();
    let decoded = decode(described).unwrap();
    assert_eq!(decoded.identity.basis, Some(IdentityBasis::Description));

    let mut geometry_only = test_ref_entry();
    geometry_only.identity.role = role.as_ptr();
    geometry_only.geometry.bounds = crate::types::AdRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    };
    geometry_only.geometry.has_bounds = true;
    geometry_only.geometry.bounds_space = if cfg!(target_os = "linux") {
        crate::AD_BOUNDS_SPACE_WINDOW
    } else {
        crate::AD_BOUNDS_SPACE_SCREEN
    };
    let decoded = decode(geometry_only).unwrap();
    assert_eq!(decoded.identity.basis, Some(IdentityBasis::Positional));
}

#[test]
fn every_decoded_entry_passes_the_identity_basis_gate() {
    let role = CString::new("button").unwrap();
    let native = CString::new("row-7").unwrap();
    let mut entries = Vec::new();
    let mut identified = test_ref_entry();
    identified.identity.role = role.as_ptr();
    identified.identity.native_id = native.as_ptr();
    entries.push(identified);
    let mut named = test_ref_entry();
    named.identity.role = role.as_ptr();
    named.identity.name = c"target".as_ptr();
    entries.push(named);
    let mut geometry_only = test_ref_entry();
    geometry_only.identity.role = role.as_ptr();
    geometry_only.geometry.bounds = crate::types::AdRect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
    };
    geometry_only.geometry.has_bounds = true;
    geometry_only.geometry.bounds_space = if cfg!(target_os = "linux") {
        crate::AD_BOUNDS_SPACE_WINDOW
    } else {
        crate::AD_BOUNDS_SPACE_SCREEN
    };
    entries.push(geometry_only);

    for entry in entries {
        let decoded = decode(entry).unwrap();
        assert!(decoded.identity.basis.is_some());
        assert!(
            agent_desktop_core::refs::identity_basis_gate::basis_gate(&decoded.identity).is_ok()
        );
    }
}
