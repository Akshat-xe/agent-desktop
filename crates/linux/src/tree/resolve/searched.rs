use super::candidate::Candidate;

/// What a search over a finished walk found and whether any region of the
/// tree stayed unread.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Searched {
    pub(crate) candidates: Vec<Candidate>,
    /// Live nodes that answered only part of the stored evidence. They can never
    /// be resolved to, but an identity ref must count them before it settles.
    pub(crate) partial: usize,
    pub(crate) unread_region: bool,
}

impl Searched {
    /// Every live node that answered the stored evidence, resolvable or not. A ref that cannot
    /// select one element among them is refused.
    pub(crate) fn answers(&self) -> usize {
        self.candidates.len() + self.partial
    }
}
