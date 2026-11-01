pub(super) struct EntryPointResolve<'a> {
    pub(super) ref_id: &'a str,
    pub(super) entry: &'a crate::RefEntry,
    pub(super) stability: Option<Option<u64>>,
    pub(super) lease: Option<&'a crate::InteractionLease>,
    pub(super) verify_receives_events: bool,
}
