use std::os::raw::c_char;

#[repr(C)]
pub struct AdRefScope {
    /// The drill-down root that created the entry. An exact-ref entry that sets it is refused
    /// with `STALE_REF`, because the root's snapshot is not available to resolve against.
    pub root_ref: *const c_char,
    pub path: *const u32,
    pub path_count: usize,
    pub path_is_absolute: bool,
}

pub const AD_REF_SCOPE_SIZE: usize = 32;

const _: () = assert!(std::mem::size_of::<AdRefScope>() == AD_REF_SCOPE_SIZE);
