//! The snapshot's count of look-alikes, which decides what evidence, if any,
//! singles one ref out from every element that resembles it.

use crate::refs::identity_basis_unread::UnreadFields;
use crate::{ElementIdentifier, IdentifierKind, IdentityBasis, RefEntryIdentity};
use std::collections::HashMap;

type IdentifierKey = (String, IdentifierKind, String);
type TextKey = (String, Option<(IdentifierKind, String)>, String);
type RoleTextKey = (String, String);

/// Counts every identifier, stable name and description among the entries a
/// snapshot allocated, so each entry can learn which of them is unique. A count
/// proves uniqueness only where the survey read that field on every element of
/// the entry's role: `mark_incomplete` withdraws every proof and `mark_unread`
/// the proofs that rest on the fields it names for one role. With identifiers
/// unread, a name or description unique across the whole role still proves
/// itself, because the live predicate is never broader than the role.
#[derive(Default)]
pub(crate) struct IdentityBasisCounts {
    identifiers: HashMap<IdentifierKey, usize>,
    names: HashMap<TextKey, usize>,
    descriptions: HashMap<TextKey, usize>,
    names_by_role: HashMap<RoleTextKey, usize>,
    descriptions_by_role: HashMap<RoleTextKey, usize>,
    unread: UnreadFields,
}

impl IdentityBasisCounts {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn mark_incomplete(&mut self) {
        self.unread.mark_all();
    }

    pub(crate) fn mark_unread(
        &mut self,
        role: &str,
        identifiers: bool,
        names: bool,
        descriptions: bool,
    ) {
        self.unread.mark(role, identifiers, names, descriptions);
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
                .names_by_role
                .entry((identity.role.clone(), name.to_string()))
                .or_default() += 1;
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
                .descriptions_by_role
                .entry((identity.role.clone(), description.to_string()))
                .or_default() += 1;
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
        if self.unread.all() {
            return unproven_basis(identity);
        }
        let role = identity.role.as_str();
        let identifier = native_identifier(identity);
        let identifiers_read = self.unread.identifiers_read(role);
        if let Some(identifier) = identifier.filter(|_| identifiers_read) {
            let key = (role.to_owned(), identifier.kind, identifier.value.clone());
            if self.identifiers.get(&key).copied().unwrap_or_default() == 1 {
                return IdentityBasis::NativeId;
            }
        }
        let group = identifier.map(|identifier| (identifier.kind, identifier.value.clone()));
        let unique_text = |text: &str,
                           grouped: &HashMap<TextKey, usize>,
                           by_role: &HashMap<RoleTextKey, usize>| {
            let count = if identifiers_read {
                grouped.get(&(role.to_owned(), group.clone(), text.to_owned()))
            } else {
                by_role.get(&(role.to_owned(), text.to_owned()))
            };
            count.copied().unwrap_or_default() == 1
        };
        if let Some(name) = stable_name(identity).filter(|_| self.unread.names_read(role)) {
            if unique_text(name, &self.names, &self.names_by_role) {
                return IdentityBasis::Name;
            }
        }
        let description = crate::tree::accname::non_blank(identity.description.as_deref());
        if let Some(description) = description.filter(|_| self.unread.descriptions_read(role)) {
            if unique_text(description, &self.descriptions, &self.descriptions_by_role) {
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
