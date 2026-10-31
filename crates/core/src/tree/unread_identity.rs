/// The identity fields an observation could not read on one element. An
/// unread field is not an absent one: the value it would have shown can match
/// the same field a proof relies on, so the census withdraws that field's
/// proofs for the element's role rather than counting it as no look-alike.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnreadIdentity {
    pub role: bool,
    pub identifiers: bool,
    pub names: bool,
    pub values: bool,
    pub descriptions: bool,
}
