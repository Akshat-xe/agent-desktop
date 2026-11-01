//! Same-call confirmation of a locator anchor against every field it observed.

use crate::{
    IdentityMatch,
    locator::live_locator::{IdentifierEvidence, LocatorField},
    refs::{
        RefEntry,
        ref_identity::{
            identifier_evidence_match, meaningful_field, same_field_match, stable_name,
            stable_name_field, stable_value, stable_value_field,
        },
    },
};

/// Confirms a candidate against every stable field the entry recorded, each compared with the
/// same live field. An unrecorded field cannot decide, and an unread live field is `Unknown`.
pub(crate) fn observed_identity_match(
    entry: &RefEntry,
    actual_name: &LocatorField<String>,
    actual_value: &LocatorField<String>,
    actual_description: &LocatorField<String>,
    actual_identifiers: &IdentifierEvidence,
) -> IdentityMatch {
    let role = entry.identity.role.as_str();
    let verdicts = [
        entry
            .identity
            .native_id
            .as_ref()
            .map(|expected| identifier_evidence_match(expected, actual_identifiers)),
        stable_name(
            role,
            entry.identity.name.as_deref(),
            entry.identity.value.as_deref(),
        )
        .map(|expected| {
            same_field_match(expected, stable_name_field(role, actual_name, actual_value))
        }),
        stable_value(role, entry.identity.value.as_deref())
            .map(|expected| same_field_match(expected, stable_value_field(role, actual_value))),
        crate::tree::accname::non_blank(entry.identity.description.as_deref())
            .map(|expected| same_field_match(expected, meaningful_field(actual_description))),
    ];
    let mut verdicts = verdicts.into_iter().flatten();
    let Some(first) = verdicts.next() else {
        return IdentityMatch::Unknown;
    };
    verdicts.fold(first, merge_identity_match)
}

fn merge_identity_match(left: IdentityMatch, right: IdentityMatch) -> IdentityMatch {
    match (left, right) {
        (IdentityMatch::NoMatch, _) | (_, IdentityMatch::NoMatch) => IdentityMatch::NoMatch,
        (IdentityMatch::Unknown, _) | (_, IdentityMatch::Unknown) => IdentityMatch::Unknown,
        _ => IdentityMatch::Match,
    }
}
