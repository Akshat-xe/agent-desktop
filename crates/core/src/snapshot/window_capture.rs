use crate::{
    AppError, WindowInfo,
    adapter::{PlatformAdapter, TreeOptions},
    locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
};

/// One window capture: the observation every ref is counted against, plus the
/// logical depth the presented tree is cut at when that observation reaches
/// deeper than the caller asked for.
pub(crate) struct WindowCapture {
    pub(crate) observed: ObservedTree,
    presentation_depth: Option<u8>,
    /// Why the whole-window read that would prove this capture's tokenless
    /// refs failed or came back cut short, so those refs stay unproven.
    pub(crate) identity_unproven: Option<String>,
}

impl WindowCapture {
    pub(crate) fn into_parts(
        self,
    ) -> Result<(crate::AccessibilityNode, bool, usize, ObservedTree), AppError> {
        let Self {
            mut observed,
            presentation_depth,
            ..
        } = self;
        let (tree, complete, nodes_observed) = match presentation_depth {
            Some(depth) => observed.project_window(depth)?,
            None => observed.project_partial()?,
        };
        Ok((tree, complete, nodes_observed, observed))
    }
}

/// Observes one window, and for a presentation whose shallow walk leaves refs
/// that can only re-identify themselves by their recorded identity, observes
/// the whole window instead and presents a cut of that one observation.
///
/// A shallow walk never sees the nodes below its depth, so the census that
/// proves a ref unique cannot run over it, and every ref it allocates is
/// refused at resolution. A ref carrying an element token re-identifies itself,
/// so a capture made of those keeps its single shallow walk. A whole-window
/// read that fails or comes back incomplete keeps that shallow tree and proves
/// nothing, rather than counting a narrower window or failing a capture whose
/// shallow walk already succeeded.
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
            identity_unproven: None,
        });
    }
    let unproven = match crate::refs::resolve_saved_ref::observe_whole_window(
        adapter, opts, window, deadline, None,
    ) {
        Ok(whole) if whole.is_exhaustive() => {
            return Ok(WindowCapture {
                observed: whole,
                presentation_depth: Some(depth),
                identity_unproven: None,
            });
        }
        Ok(_) => "the whole-window read was cut short".to_owned(),
        Err(error) => error.message,
    };
    Ok(WindowCapture {
        observed,
        presentation_depth: None,
        identity_unproven: Some(unproven),
    })
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

/// Whether the presentation allocation builds from this observation keeps an
/// entry that can only re-identify itself through the capture proof: a ref or a
/// skeleton drill anchor whose identity carries no element token. Only a whole
/// window proves such an entry unique, so a capture that would present one is
/// re-observed. A presentation whose selected entries all carry a token resolves
/// through the tokens and keeps its single shallow walk.
fn needs_whole_window(observed: &ObservedTree) -> bool {
    if observed.is_exhaustive() {
        return false;
    }
    let mut presented = observed.clone();
    presented
        .project_partial()
        .is_ok_and(|(presented, _, _)| crate::refs::ref_alloc::selects_tokenless_entry(&presented))
}

#[cfg(test)]
#[path = "window_capture_tests.rs"]
mod tests;
