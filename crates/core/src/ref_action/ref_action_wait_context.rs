use crate::{CommandContext, PlatformAdapter, RefEntry};

#[derive(Clone, Copy)]
pub(crate) struct RefActionWaitContext<'a> {
    pub(crate) adapter: &'a dyn PlatformAdapter,
    pub(crate) entry: &'a RefEntry,
    pub(crate) ref_id: &'a str,
    pub(crate) context: &'a CommandContext,
}

impl RefActionWaitContext<'_> {
    pub(crate) fn resolve(
        self,
        deadline: crate::Deadline,
    ) -> Result<crate::NativeHandle, crate::AdapterError> {
        crate::refs::ref_resolve::resolve(
            self.adapter,
            self.entry,
            self.ref_id,
            self.context,
            deadline,
        )
    }
}
