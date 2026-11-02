use super::action_list::{AvailableActionsRead, read_settable};
use crate::tree::AXElement;
use crate::tree::properties::attributes::{copy_bool_attr_result, copy_element_attr_result};
use std::cell::RefCell;

const VERDICT_CACHE_LEN: usize = 8;

type Verdicts = Vec<(AXElement, Option<bool>)>;

thread_local! {
    static VERDICTS: RefCell<Option<Verdicts>> = const { RefCell::new(None) };
}

/// Shares selection-owner answers among siblings for one observation only;
/// any read outside an observation computes a fresh answer.
pub(crate) struct ObservationVerdicts;

impl ObservationVerdicts {
    pub(crate) fn begin() -> Self {
        VERDICTS.with(|cache| *cache.borrow_mut() = Some(Vec::new()));
        Self
    }
}

impl Drop for ObservationVerdicts {
    fn drop(&mut self) {
        VERDICTS.with(|cache| *cache.borrow_mut() = None);
    }
}

pub(crate) const MAX_SELECTION_OWNER_DEPTH: usize = 2;

/// A row with no press action of its own is still clickable when an enabled
/// ancestor lets its selection be written; such a member is a list item, not a
/// control, so its own `AXEnabled` says nothing about whether it can be chosen.
/// Siblings share their ancestors, so within an observation the answer is kept
/// per parent.
pub(super) fn container_owns_selection(
    el: &AXElement,
    deadline: std::time::Instant,
    read: &mut AvailableActionsRead,
) -> bool {
    let Ok(Some(parent)) = copy_element_attr_result(el, "AXParent", deadline) else {
        return false;
    };
    let same = crate::tree::properties::capabilities::same_element;
    let known = VERDICTS.with(|cache| {
        cache
            .borrow()
            .as_ref()?
            .iter()
            .find(|(seen, _)| same(seen, &parent))
            .map(|(_, verdict)| *verdict)
    });
    let verdict = known.unwrap_or_else(|| {
        let verdict = owner_from(&parent, deadline, read);
        if read.complete {
            VERDICTS.with(|cache| {
                if let Some(cache) = cache.borrow_mut().as_mut() {
                    if cache.len() >= VERDICT_CACHE_LEN {
                        cache.remove(0);
                    }
                    cache.push((parent.clone(), verdict));
                }
            });
        }
        verdict
    });
    read.selects_through_container = verdict == Some(true);
    read.selects_through_container
}

/// `Some(enabled)` when `first` or an ancestor within the depth bound owns a
/// settable selection.
fn owner_from(
    first: &AXElement,
    deadline: std::time::Instant,
    read: &mut AvailableActionsRead,
) -> Option<bool> {
    let mut current = first.clone();
    for level in 0..MAX_SELECTION_OWNER_DEPTH {
        let owns = crate::actions::select::container_select::SELECTION_ATTRIBUTES
            .iter()
            .any(|attr| read_settable(&current, attr, deadline, read).unwrap_or(false));
        if owns {
            return Some(copy_bool_attr_result(&current, "AXEnabled", deadline) != Ok(Some(false)));
        }
        if level + 1 < MAX_SELECTION_OWNER_DEPTH {
            current = copy_element_attr_result(&current, "AXParent", deadline).ok()??;
        }
    }
    None
}

/// True when writing the container's selection is the only way to choose `el`,
/// so a press on anything inside it would act on something else.
pub(crate) fn selects_only_through_container(el: &AXElement, deadline: std::time::Instant) -> bool {
    let mut read = AvailableActionsRead::default();
    let own = crate::actions::select::container_select::SELECTED;
    !read_settable(el, own, deadline, &mut read).unwrap_or(false)
        && container_owns_selection(el, deadline, &mut read)
}
