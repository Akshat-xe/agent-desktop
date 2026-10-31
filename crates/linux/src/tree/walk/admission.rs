/// What a search does with one node it has read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Admission {
    /// The node answers to the stored evidence and counts as a candidate.
    Collect,
    /// The node answers only part of the stored evidence: it holds the stored
    /// object while the identity the ref recorded no longer agrees, or it
    /// carries that identity while the stored object is gone. It counts as a
    /// live element the ref could name, so a second answer makes the ref
    /// ambiguous, but it is never resolved to, because nothing proves it is the
    /// element the ref was taken from.
    Partial,
    /// The node's evidence could not settle the question, so this region of
    /// the tree was not read conclusively and a negative verdict is withheld.
    Unread,
    /// The node is refuted or its role rules it out.
    Reject,
}
