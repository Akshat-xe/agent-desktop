/// What a search does with one node it has read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Admission {
    /// The node answers to the stored evidence and counts as a candidate.
    Collect,
    /// The node carries the identity the ref recorded while the stored object address names
    /// another element: it makes a second match ambiguous, but is never resolved to.
    Partial,
    /// The node's evidence could not settle the question, so this region of
    /// the tree was not read conclusively and a negative verdict is withheld.
    Unread,
    /// The node is refuted or its role rules it out.
    Reject,
}
