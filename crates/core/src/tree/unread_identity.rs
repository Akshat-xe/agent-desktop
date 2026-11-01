/// The identity fields an observation could not read on one element. An unread field is not an
/// absent one, so the census withdraws the proofs that rest on it for the element's role.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnreadIdentity {
    pub role: bool,
    pub identifiers: bool,
    pub names: bool,
    pub values: bool,
    pub descriptions: bool,
}
