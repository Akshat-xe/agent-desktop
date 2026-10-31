use super::*;
use crate::ErrorCode;

fn entry_with(basis: Option<IdentityBasis>) -> RefEntryIdentity {
    RefEntryIdentity {
        role: "button".into(),
        basis,
        ..RefEntryIdentity::default()
    }
}

#[test]
fn a_legacy_entry_is_stale_and_an_ambiguous_one_is_refused() {
    let legacy = basis_gate(&entry_with(None)).unwrap_err();
    assert_eq!(legacy.code, ErrorCode::StaleRef);
    let ambiguous = basis_gate(&entry_with(Some(IdentityBasis::Ambiguous))).unwrap_err();
    assert_eq!(ambiguous.code, ErrorCode::AmbiguousTarget);
    for basis in [
        IdentityBasis::NativeId,
        IdentityBasis::Name,
        IdentityBasis::Description,
        IdentityBasis::Positional,
    ] {
        assert!(basis_gate(&entry_with(Some(basis))).is_ok());
    }
}

#[test]
fn only_positional_refs_let_bounds_decide() {
    assert!(bounds_may_decide(&entry_with(Some(
        IdentityBasis::Positional
    ))));
    assert!(!requires_live_uniqueness(&entry_with(Some(
        IdentityBasis::Positional
    ))));
    for basis in [
        IdentityBasis::NativeId,
        IdentityBasis::Name,
        IdentityBasis::Description,
    ] {
        assert!(requires_live_uniqueness(&entry_with(Some(basis))));
        assert!(!bounds_may_decide(&entry_with(Some(basis))));
    }
    assert!(!bounds_may_decide(&entry_with(None)));
}
