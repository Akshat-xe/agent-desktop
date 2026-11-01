use std::hash::{Hash, Hasher};

use crate::{
    AccessibilityNode, ElementIdentifier, IdentityMatch, RefEntry,
    locator::live_locator::{LocatorEvidence, LocatorField},
    refs::ref_identity::{equality_match, meaningful_field, stable_name, stable_name_field},
    tree::accname::non_blank,
};

const NATIVE_ID: u8 = 0b001;
const NAME: u8 = 0b010;
const DESCRIPTION: u8 = 0b100;

/// The identity fields capture and resolution both compare: role plus whichever
/// of native identifier, stable name and description an element records.
/// Mutable values and bounds are never identity.
#[derive(Clone, Copy)]
pub(crate) struct FullPredicate<'a> {
    role: &'a str,
    native_id: Option<&'a ElementIdentifier>,
    name: Option<&'a str>,
    description: Option<&'a str>,
    mask: u8,
}

impl<'a> FullPredicate<'a> {
    pub(crate) fn of_node(node: &'a AccessibilityNode) -> Self {
        Self::of_fields(
            &node.role,
            node.identity.name.as_deref(),
            node.identity.value.as_deref(),
            node.identity.description.as_deref(),
            node.identity.native_id.as_ref(),
        )
    }

    pub(crate) fn of_entry(entry: &'a RefEntry) -> Self {
        Self::of_fields(
            &entry.identity.role,
            entry.identity.name.as_deref(),
            entry.identity.value.as_deref(),
            entry.identity.description.as_deref(),
            entry.identity.native_id.as_ref(),
        )
    }

    fn of_fields(
        role: &'a str,
        name: Option<&'a str>,
        value: Option<&'a str>,
        description: Option<&'a str>,
        native_id: Option<&'a ElementIdentifier>,
    ) -> Self {
        Self {
            role,
            native_id: native_id.filter(|identifier| !identifier.value.trim().is_empty()),
            name: stable_name(role, name, value),
            description: non_blank(description),
            mask: 0,
        }
        .recorded()
    }

    /// The one rule capture and resolution apply: every field the predicate
    /// records must equal the live element's, and a field the live read could
    /// not answer is unknown instead of matching.
    pub(crate) fn compare(&self, live: &LivePredicate<'_>) -> IdentityMatch {
        [
            field_match(Some(self.role), &live.role),
            self.native_id_match(live),
            field_match(
                self.name,
                &stable_name_field(self.role, &live.name, &live.value),
            ),
            field_match(self.description, &meaningful_field(&live.description)),
        ]
        .into_iter()
        .fold(IdentityMatch::Match, conjunction)
    }

    fn native_id_match(&self, live: &LivePredicate<'_>) -> IdentityMatch {
        let Some(recorded) = self.native_id else {
            return IdentityMatch::Match;
        };
        if live.native_ids.contains(recorded) {
            IdentityMatch::Match
        } else if live.native_ids_complete {
            IdentityMatch::NoMatch
        } else {
            IdentityMatch::Unknown
        }
    }

    /// The fields this predicate keeps; capture counts by subsets of them, so
    /// two predicates only compare once they keep the same fields.
    pub(crate) fn mask(&self) -> u8 {
        self.mask
    }

    fn recorded(mut self) -> Self {
        self.mask = u8::from(self.native_id.is_some())
            | (u8::from(self.name.is_some()) << 1)
            | (u8::from(self.description.is_some()) << 2);
        self
    }

    /// The predicate narrowed to the fields `mask` selects: capture counts each
    /// field subset a recorded entry may be looked up by.
    pub(crate) fn masked(&self, mask: u8) -> Self {
        Self {
            role: self.role,
            native_id: (mask & NATIVE_ID != 0).then_some(self.native_id).flatten(),
            name: (mask & NAME != 0).then_some(self.name).flatten(),
            description: (mask & DESCRIPTION != 0)
                .then_some(self.description)
                .flatten(),
            mask,
        }
    }
}

/// Capture groups elements by this predicate, so equality is the same rule
/// resolution applies, once both sides keep the same fields.
impl PartialEq for FullPredicate<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.mask == other.mask
            && self.compare(&LivePredicate::of_predicate(other)) == IdentityMatch::Match
    }
}

impl Eq for FullPredicate<'_> {}

impl Hash for FullPredicate<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.role.hash(state);
        self.native_id.hash(state);
        self.name.hash(state);
        self.description.hash(state);
        self.mask.hash(state);
    }
}

/// What a live element reports for the fields a predicate records, with the
/// identifier read's completeness kept so an unread field answers `Unknown`.
pub(crate) struct LivePredicate<'a> {
    role: LocatorField<&'a str>,
    name: LocatorField<&'a str>,
    value: LocatorField<&'a str>,
    description: LocatorField<&'a str>,
    native_ids: &'a [ElementIdentifier],
    native_ids_complete: bool,
}

impl<'a> LivePredicate<'a> {
    pub(crate) fn observed(evidence: &'a LocatorEvidence) -> Self {
        Self {
            role: meaningful_field(&evidence.role),
            name: meaningful_field(&evidence.name),
            value: meaningful_field(&evidence.value),
            description: meaningful_field(&evidence.description),
            native_ids: evidence.identifiers.identifiers(),
            native_ids_complete: evidence.identifiers.is_complete(),
        }
    }

    fn of_predicate<'b: 'a>(predicate: &'a FullPredicate<'b>) -> Self {
        Self {
            role: LocatorField::Known(predicate.role),
            name: field_of(predicate.name),
            value: LocatorField::Absent,
            description: field_of(predicate.description),
            native_ids: predicate.native_id.map(std::slice::from_ref).unwrap_or(&[]),
            native_ids_complete: true,
        }
    }
}

fn field_of(value: Option<&str>) -> LocatorField<&str> {
    value
        .map(LocatorField::Known)
        .unwrap_or(LocatorField::Absent)
}

fn field_match(expected: Option<&str>, actual: &LocatorField<&str>) -> IdentityMatch {
    match expected {
        Some(expected) => match actual {
            LocatorField::Known(actual) => equality_match(expected, actual),
            LocatorField::Absent => IdentityMatch::NoMatch,
            LocatorField::Unknown => IdentityMatch::Unknown,
        },
        None => IdentityMatch::Match,
    }
}

fn conjunction(left: IdentityMatch, right: IdentityMatch) -> IdentityMatch {
    match (left, right) {
        (IdentityMatch::NoMatch, _) | (_, IdentityMatch::NoMatch) => IdentityMatch::NoMatch,
        (IdentityMatch::Unknown, _) | (_, IdentityMatch::Unknown) => IdentityMatch::Unknown,
        _ => IdentityMatch::Match,
    }
}
