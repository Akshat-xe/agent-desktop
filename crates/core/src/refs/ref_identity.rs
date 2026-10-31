use crate::{
    IdentityBasis, IdentityMatch,
    locator::live_locator::{IdentifierEvidence, LocatorField},
    refs::RefEntry,
    tree::roles::is_mutable_value_role,
};

/// Whether the entry records evidence that singled it out from its look-alikes
/// when it was captured. A positional entry, an entry whose evidence was not
/// unique, and an entry written before bases were recorded carry none.
/// Whether the entry carries identity a resolver can verify. A positional
/// entry is judged by its own fields, which a snapshot's positional ref never
/// has and a locator anchor observed in the same call may.
pub fn has_meaningful_identity(entry: &RefEntry) -> bool {
    match entry.identity.basis {
        Some(IdentityBasis::NativeId | IdentityBasis::Name | IdentityBasis::Description) => true,
        Some(IdentityBasis::Positional) => identity_fields_are_meaningful(
            entry.identity.role.as_str(),
            entry.identity.name.as_deref(),
            entry.identity.value.as_deref(),
            entry.identity.description.as_deref(),
            entry
                .identity
                .native_id
                .as_ref()
                .map(|identifier| identifier.value.as_str()),
        ),
        None | Some(IdentityBasis::Ambiguous) => false,
    }
}

pub fn has_stable_text_identity(entry: &RefEntry) -> bool {
    stable_name(
        entry.identity.role.as_str(),
        entry.identity.name.as_deref(),
        entry.identity.value.as_deref(),
    )
    .is_some()
        || stable_value(
            entry.identity.role.as_str(),
            entry.identity.value.as_deref(),
        )
        .is_some()
        || crate::tree::accname::non_blank(entry.identity.description.as_deref()).is_some()
}

/// Whether an element's own identifier or stable text identifies it, ignoring
/// any retained object. A platform that can only retain a positional object
/// path must not present that path as identity evidence for a node its
/// identifier or text already names, because the path moves when the siblings
/// are reordered.
pub fn identity_fields_are_meaningful(
    role: &str,
    name: Option<&str>,
    value: Option<&str>,
    description: Option<&str>,
    native_id: Option<&str>,
) -> bool {
    crate::tree::accname::non_blank(native_id).is_some()
        || stable_name(role, name, value).is_some()
        || stable_value(role, value).is_some()
        || crate::tree::accname::non_blank(description).is_some()
}

/// Whether a live element still carries the evidence the entry recorded.
///
/// The recorded basis decides which evidence counts. A native-id entry matches
/// on identifier kind and value alone. A name or description entry matches on
/// that text and, when the entry also has an identifier, on the identifier too.
/// A positional entry keeps the text rules it had before bases existed. An
/// entry without a basis, and an ambiguous one, never matches: only the basis
/// gate may decide how they resolve. Roles are checked by the resolver, and
/// control values never decide a match.
pub fn identity_match(
    entry: &RefEntry,
    actual_name: &LocatorField<String>,
    actual_value: &LocatorField<String>,
    actual_description: &LocatorField<String>,
    actual_identifiers: &IdentifierEvidence,
) -> IdentityMatch {
    match entry.identity.basis {
        Some(IdentityBasis::NativeId) => native_identity_match(entry, actual_identifiers),
        Some(IdentityBasis::Name) => {
            name_identity_match(entry, actual_name, actual_value, actual_identifiers)
        }
        Some(IdentityBasis::Description) => description_identity_match(
            entry,
            actual_name,
            actual_value,
            actual_description,
            actual_identifiers,
        ),
        Some(IdentityBasis::Positional) => match entry.identity.native_id.as_ref() {
            Some(expected) => identifier_evidence_match(expected, actual_identifiers),
            None => stable_text_match(entry, actual_name, actual_value, actual_description),
        },
        None | Some(IdentityBasis::Ambiguous) => IdentityMatch::NoMatch,
    }
}

fn native_identity_match(
    entry: &RefEntry,
    actual_identifiers: &IdentifierEvidence,
) -> IdentityMatch {
    match entry.identity.native_id.as_ref() {
        Some(expected) => identifier_evidence_match(expected, actual_identifiers),
        None => IdentityMatch::NoMatch,
    }
}

fn name_identity_match(
    entry: &RefEntry,
    actual_name: &LocatorField<String>,
    actual_value: &LocatorField<String>,
    actual_identifiers: &IdentifierEvidence,
) -> IdentityMatch {
    let role = entry.identity.role.as_str();
    let Some(expected) = stable_name(
        role,
        entry.identity.name.as_deref(),
        entry.identity.value.as_deref(),
    ) else {
        return IdentityMatch::NoMatch;
    };
    text_identity_match(
        expected,
        entry.identity.native_id.as_ref(),
        stable_name_field(role, actual_name, actual_value),
        stable_value_field(role, actual_value),
        actual_identifiers,
    )
}

fn description_identity_match(
    entry: &RefEntry,
    actual_name: &LocatorField<String>,
    actual_value: &LocatorField<String>,
    actual_description: &LocatorField<String>,
    actual_identifiers: &IdentifierEvidence,
) -> IdentityMatch {
    let Some(expected) = crate::tree::accname::non_blank(entry.identity.description.as_deref())
    else {
        return IdentityMatch::NoMatch;
    };
    text_identity_match(
        expected,
        entry.identity.native_id.as_ref(),
        meaningful_field(actual_description),
        stable_name_field(entry.identity.role.as_str(), actual_name, actual_value),
        actual_identifiers,
    )
}

fn text_identity_match(
    expected_text: &str,
    expected_identifier: Option<&crate::ElementIdentifier>,
    actual_primary: LocatorField<&str>,
    actual_fallback: LocatorField<&str>,
    actual_identifiers: &IdentifierEvidence,
) -> IdentityMatch {
    if let Some(expected) = expected_identifier {
        match identifier_evidence_match(expected, actual_identifiers) {
            IdentityMatch::Match => {}
            refusal => return refusal,
        }
    }
    match_primary_identity(expected_text, actual_primary, actual_fallback)
}

/// Whether a live element still carries one typed identifier. An incomplete
/// identifier read never proves a non-match, so it reports `Unknown`.
pub fn identifier_evidence_match(
    expected: &crate::ElementIdentifier,
    actual_identifiers: &IdentifierEvidence,
) -> IdentityMatch {
    let Some(expected_value) = crate::tree::accname::non_blank(Some(expected.value.as_str()))
    else {
        return IdentityMatch::NoMatch;
    };
    if actual_identifiers
        .identifiers()
        .iter()
        .any(|actual| actual.kind == expected.kind && actual.value == expected_value)
    {
        return IdentityMatch::Match;
    }
    if actual_identifiers.is_complete() {
        IdentityMatch::NoMatch
    } else {
        IdentityMatch::Unknown
    }
}

#[cfg(test)]
pub(crate) fn identity_matches(
    entry: &RefEntry,
    actual_name: Option<&str>,
    actual_value: Option<&str>,
    actual_description: Option<&str>,
    actual_native_id: Option<&str>,
) -> bool {
    let actual_name = option_field(actual_name);
    let actual_value = option_field(actual_value);
    let actual_description = option_field(actual_description);
    let identifiers = IdentifierEvidence::typed(
        actual_native_id
            .into_iter()
            .map(|value| crate::ElementIdentifier {
                kind: crate::IdentifierKind::AxIdentifier,
                value: value.to_string(),
            }),
        actual_native_id.map(|_| 0),
        true,
    );
    identity_match(
        entry,
        &actual_name,
        &actual_value,
        &actual_description,
        &identifiers,
    ) == IdentityMatch::Match
}

/// Whether one element's stable text carves a live identity that can refute
/// or corroborate the stored one.
///
/// Public because the Windows resolver composes it with `identity_match` to
/// keep its AutomationId-first corroboration rule: an id that still resolves
/// onto a candidate whose stable text drifted must be refuted, the A7-3
/// wrong-target shape. It reads the entry's stable text only, never the
/// recorded basis, so a caller that gates on the basis can still ask whether
/// the live text corroborates a candidate.
pub fn stable_text_match(
    entry: &RefEntry,
    actual_name: &LocatorField<String>,
    actual_value: &LocatorField<String>,
    actual_description: &LocatorField<String>,
) -> IdentityMatch {
    let expected_name = stable_name(
        entry.identity.role.as_str(),
        entry.identity.name.as_deref(),
        entry.identity.value.as_deref(),
    );
    let expected_value = stable_value(
        entry.identity.role.as_str(),
        entry.identity.value.as_deref(),
    );
    let expected_description =
        crate::tree::accname::non_blank(entry.identity.description.as_deref());
    let actual_name = stable_name_field(entry.identity.role.as_str(), actual_name, actual_value);
    let actual_value = stable_value_field(entry.identity.role.as_str(), actual_value);
    let actual_description = meaningful_field(actual_description);

    if let Some(expected) = expected_name {
        return match_primary_identity(expected, actual_name, actual_value);
    }
    if let Some(expected) = expected_value {
        return match_primary_identity(expected, actual_value, actual_name);
    }
    if let Some(expected) = expected_description {
        return match_primary_identity(expected, actual_description, actual_name);
    }
    if is_mutable_value_role(entry.identity.role.as_str()) {
        return IdentityMatch::Unknown;
    }
    empty_identity_match([actual_name, actual_value, actual_description])
}

fn match_primary_identity(
    expected: &str,
    actual_primary: LocatorField<&str>,
    actual_fallback: LocatorField<&str>,
) -> IdentityMatch {
    match actual_primary {
        LocatorField::Known(actual) => equality_match(expected, actual),
        LocatorField::Unknown => IdentityMatch::Unknown,
        LocatorField::Absent => match actual_fallback {
            LocatorField::Known(actual) => equality_match(expected, actual),
            LocatorField::Unknown => IdentityMatch::Unknown,
            LocatorField::Absent => IdentityMatch::NoMatch,
        },
    }
}

fn equality_match(expected: &str, actual: &str) -> IdentityMatch {
    if actual == expected {
        IdentityMatch::Match
    } else {
        IdentityMatch::NoMatch
    }
}

fn empty_identity_match(fields: [LocatorField<&str>; 3]) -> IdentityMatch {
    if fields.iter().any(LocatorField::is_unknown) {
        return IdentityMatch::Unknown;
    }
    if fields
        .iter()
        .any(|field| matches!(field, LocatorField::Known(_)))
    {
        IdentityMatch::NoMatch
    } else {
        IdentityMatch::Unknown
    }
}

#[cfg(test)]
fn option_field(value: Option<&str>) -> LocatorField<String> {
    value
        .map(str::to_string)
        .map(LocatorField::Known)
        .unwrap_or(LocatorField::Absent)
}

fn meaningful_field(field: &LocatorField<String>) -> LocatorField<&str> {
    match field {
        LocatorField::Known(value) => crate::tree::accname::non_blank(Some(value.as_str()))
            .map(LocatorField::Known)
            .unwrap_or(LocatorField::Absent),
        LocatorField::Absent => LocatorField::Absent,
        LocatorField::Unknown => LocatorField::Unknown,
    }
}

fn stable_name_field<'a>(
    role: &str,
    name: &'a LocatorField<String>,
    value: &LocatorField<String>,
) -> LocatorField<&'a str> {
    let name = meaningful_field(name);
    if !is_mutable_value_role(role) {
        return name;
    }
    let LocatorField::Known(name) = name else {
        return name;
    };
    match meaningful_field(value) {
        LocatorField::Known(value) if value_matches_name(Some(value), name) => LocatorField::Absent,
        LocatorField::Known(_) | LocatorField::Absent => LocatorField::Known(name),
        LocatorField::Unknown => LocatorField::Unknown,
    }
}

fn stable_value_field<'a>(role: &str, value: &'a LocatorField<String>) -> LocatorField<&'a str> {
    if is_mutable_value_role(role) {
        LocatorField::Absent
    } else {
        meaningful_field(value)
    }
}

pub(crate) fn stable_name<'a>(
    role: &str,
    name: Option<&'a str>,
    value: Option<&str>,
) -> Option<&'a str> {
    let name = crate::tree::accname::non_blank(name)?;
    if is_mutable_value_role(role)
        && value_matches_name(crate::tree::accname::non_blank(value), name)
    {
        None
    } else {
        Some(name)
    }
}

fn stable_value<'a>(role: &str, value: Option<&'a str>) -> Option<&'a str> {
    (!is_mutable_value_role(role))
        .then(|| crate::tree::accname::non_blank(value))
        .flatten()
}

fn value_matches_name(value: Option<&str>, name: &str) -> bool {
    value == Some(name)
        || numeric_text(value)
            .zip(numeric_text(Some(name)))
            .is_some_and(|(value, name)| value == name)
}

fn numeric_text(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|number| number.is_finite())
}

#[cfg(test)]
#[path = "ref_identity_tests.rs"]
mod tests;
