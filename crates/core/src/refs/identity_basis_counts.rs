//! The snapshot's count of look-alikes, which decides what evidence, if any,
//! singles one ref out from every element that resembles it.

use crate::{ElementIdentifier, IdentifierKind, IdentityBasis, RefEntryIdentity};
use std::collections::HashMap;

type IdentifierKey = (String, IdentifierKind, String);
type TextKey = (String, Option<(IdentifierKind, String)>, String);

/// Counts every identifier, stable name and description among the entries a
/// snapshot allocated, so each entry can learn which of them is unique. A count
/// proves uniqueness only for a field the survey read on every element:
/// `mark_incomplete` withdraws every proof and `mark_unread` withdraws the
/// proofs that rest on the fields it names.
pub(crate) struct IdentityBasisCounts {
    identifiers: HashMap<IdentifierKey, usize>,
    names: HashMap<TextKey, usize>,
    descriptions: HashMap<TextKey, usize>,
    identifiers_read: bool,
    names_read: bool,
    descriptions_read: bool,
}

impl Default for IdentityBasisCounts {
    fn default() -> Self {
        Self {
            identifiers: HashMap::new(),
            names: HashMap::new(),
            descriptions: HashMap::new(),
            identifiers_read: true,
            names_read: true,
            descriptions_read: true,
        }
    }
}

impl IdentityBasisCounts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn mark_incomplete(&mut self) {
        self.mark_unread(true, true, true);
    }

    pub(crate) fn mark_unread(&mut self, identifiers: bool, names: bool, descriptions: bool) {
        self.identifiers_read &= !identifiers;
        self.names_read &= !names;
        self.descriptions_read &= !descriptions;
    }

    pub(crate) fn record(&mut self, identity: &RefEntryIdentity) {
        let identifier = native_identifier(identity);
        if let Some(identifier) = identifier {
            *self
                .identifiers
                .entry((
                    identity.role.clone(),
                    identifier.kind,
                    identifier.value.clone(),
                ))
                .or_default() += 1;
        }
        let identifier_group =
            identifier.map(|identifier| (identifier.kind, identifier.value.clone()));
        if let Some(name) = stable_name(identity) {
            *self
                .names
                .entry((
                    identity.role.clone(),
                    identifier_group.clone(),
                    name.to_string(),
                ))
                .or_default() += 1;
        }
        if let Some(description) = crate::tree::accname::non_blank(identity.description.as_deref())
        {
            *self
                .descriptions
                .entry((
                    identity.role.clone(),
                    identifier_group,
                    description.to_string(),
                ))
                .or_default() += 1;
        }
    }

    pub(crate) fn basis(&self, identity: &RefEntryIdentity) -> IdentityBasis {
        let identifier = native_identifier(identity);
        let name = stable_name(identity);
        let description = crate::tree::accname::non_blank(identity.description.as_deref());
        if !self.identifiers_read {
            return unproven_basis(identity);
        }
        if let Some(identifier) = identifier {
            let key = (
                identity.role.clone(),
                identifier.kind,
                identifier.value.clone(),
            );
            if self.identifiers.get(&key).copied().unwrap_or_default() == 1 {
                return IdentityBasis::NativeId;
            }
        }
        let identifier_group =
            identifier.map(|identifier| (identifier.kind, identifier.value.clone()));
        if let Some(name) = name.filter(|_| self.names_read) {
            let key = (
                identity.role.clone(),
                identifier_group.clone(),
                name.to_string(),
            );
            if self.names.get(&key).copied().unwrap_or_default() == 1 {
                return IdentityBasis::Name;
            }
        }
        if let Some(description) = description.filter(|_| self.descriptions_read) {
            let key = (
                identity.role.clone(),
                identifier_group,
                description.to_string(),
            );
            if self.descriptions.get(&key).copied().unwrap_or_default() == 1 {
                return IdentityBasis::Description;
            }
        }
        unproven_basis(identity)
    }
}

/// The basis an entry gets when nothing has proved its evidence unique: text
/// or an identifier that could match a look-alike is ambiguous, and an entry
/// with none of them can only be found by position.
pub(crate) fn unproven_basis(identity: &RefEntryIdentity) -> IdentityBasis {
    if native_identifier(identity).is_some()
        || stable_name(identity).is_some()
        || crate::tree::accname::non_blank(identity.description.as_deref()).is_some()
    {
        IdentityBasis::Ambiguous
    } else {
        IdentityBasis::Positional
    }
}

fn native_identifier(identity: &RefEntryIdentity) -> Option<&ElementIdentifier> {
    identity
        .native_id
        .as_ref()
        .filter(|identifier| !identifier.value.trim().is_empty())
}

fn stable_name(identity: &RefEntryIdentity) -> Option<&str> {
    crate::refs::ref_identity::stable_name(
        identity.role.as_str(),
        identity.name.as_deref(),
        identity.value.as_deref(),
    )
}
