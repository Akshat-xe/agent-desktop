#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityMatch {
    Match,
    NoMatch,
    Unknown,
}

impl IdentityMatch {
    pub(crate) fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::NoMatch, _) | (_, Self::NoMatch) => Self::NoMatch,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            _ => Self::Match,
        }
    }
}
