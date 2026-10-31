use crate::{
    refs::ref_alloc_options::RefAllocOptions, refs::ref_alloc_scope::RefAllocScope,
    refs::ref_alloc_source::RefAllocSource,
};

#[derive(Clone, Copy)]
pub(crate) struct RefAllocConfig<'a> {
    pub(crate) options: RefAllocOptions,
    pub(crate) source: RefAllocSource<'a>,
    pub(crate) scope: RefAllocScope<'a>,
    /// Whether the observation that produced the tree saw every node, so a
    /// single match in it is proof the element is unique.
    pub(crate) complete: bool,
}
