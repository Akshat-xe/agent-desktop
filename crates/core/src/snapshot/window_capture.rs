use crate::{
    AppError, WindowInfo,
    adapter::{PlatformAdapter, TreeOptions},
    locator::live_locator::{
        ObservationRequest, ObservationRoot, ObservedNode, ObservedTree, addressability,
    },
};

/// One window capture: the observation every ref is counted against, plus the
/// logical depth the presented tree is cut at when that observation reaches
/// deeper than the caller asked for.
pub(crate) struct WindowCapture {
    pub(crate) observed: ObservedTree,
    presentation_depth: Option<u8>,
}

impl WindowCapture {
    pub(crate) fn into_tree(self) -> Result<(crate::AccessibilityNode, bool, usize), AppError> {
        match self.presentation_depth {
            Some(depth) => Ok(self.observed.into_accessibility_tree_shallow(depth)?),
            None => Ok(self.observed.into_accessibility_tree_partial()?),
        }
    }
}

/// Observes one window, and for a presentation whose shallow walk leaves refs
/// that can only re-identify themselves by their recorded identity, observes
/// the whole window instead and presents a cut of that one observation.
///
/// A shallow walk never sees the nodes below its depth, so the census that
/// proves a ref unique cannot run over it, and every ref it allocates is
/// refused at resolution. A ref carrying an element token re-identifies itself,
/// so a capture made of those keeps its single shallow walk. A window that
/// cannot be observed completely keeps that shallow tree and proves nothing,
/// rather than counting a narrower window.
pub(crate) fn capture_window(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    window: &WindowInfo,
    deadline: crate::Deadline,
) -> Result<WindowCapture, AppError> {
    let observed = observe_window(adapter, opts, window, deadline)?;
    let depth = ObservationRequest::snapshot_logical_depth(opts);
    if depth >= crate::refs::resolve_saved_ref::WHOLE_WINDOW_DEPTH || !needs_whole_window(&observed)
    {
        return Ok(WindowCapture {
            observed,
            presentation_depth: None,
        });
    }
    match crate::refs::resolve_saved_ref::observe_whole_window(
        adapter,
        opts.surface,
        window,
        deadline,
        None,
    ) {
        Ok(whole) => Ok(WindowCapture {
            observed: whole,
            presentation_depth: Some(depth),
        }),
        Err(error) if error.code == crate::ErrorCode::Timeout => Ok(WindowCapture {
            observed,
            presentation_depth: None,
        }),
        Err(error) => Err(AppError::Adapter(error)),
    }
}

fn observe_window(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    window: &WindowInfo,
    deadline: crate::Deadline,
) -> Result<ObservedTree, AppError> {
    let observation_options = opts.with_ref_identity_bounds();
    let request = ObservationRequest::snapshot(&observation_options, deadline).validate()?;
    crate::tree::renderer_accessibility::observe_tree(
        adapter,
        ObservationRoot::Window(window),
        &request,
        None,
    )
}

/// Whether the observation leaves a ref that cannot re-identify itself without
/// the capture census: a node that will carry a ref - ref-able, or a container
/// cut short that a skeleton offers as a drill target - whose evidence carries
/// no element token, in a tree that stopped before the whole window.
fn needs_whole_window(observed: &ObservedTree) -> bool {
    let cut = !observed.is_complete() || observed.nodes.iter().any(ObservedNode::omits_descendants);
    cut && observed.nodes.iter().any(|node| {
        let token = node
            .evidence
            .identifiers
            .retained_object()
            .is_some_and(crate::refs::ref_identity::is_element_token);
        !token && (addressability(&node.evidence).0 || node.omits_descendants())
    })
}

#[cfg(test)]
#[path = "window_capture_tests.rs"]
mod tests;
