use std::collections::HashSet;

/// The fields a survey left unread, by role. A field unread on an element of
/// one role cannot hide a look-alike of another role.
#[derive(Default)]
pub(crate) struct UnreadFields {
    all: bool,
    identifiers: HashSet<String>,
    names: HashSet<String>,
    descriptions: HashSet<String>,
}

impl UnreadFields {
    pub(crate) fn mark_all(&mut self) {
        self.all = true;
    }

    pub(crate) fn mark(&mut self, role: &str, identifiers: bool, names: bool, descriptions: bool) {
        if identifiers {
            self.identifiers.insert(role.to_owned());
        }
        if names {
            self.names.insert(role.to_owned());
        }
        if descriptions {
            self.descriptions.insert(role.to_owned());
        }
    }

    pub(crate) fn all(&self) -> bool {
        self.all
    }

    pub(crate) fn identifiers_read(&self, role: &str) -> bool {
        !self.identifiers.contains(role)
    }

    pub(crate) fn names_read(&self, role: &str) -> bool {
        !self.names.contains(role)
    }

    pub(crate) fn descriptions_read(&self, role: &str) -> bool {
        !self.descriptions.contains(role)
    }
}
