use super::*;
use crate::refs::RefEntry;
use std::{sync::Mutex, time::Duration};

struct TimedResolver {
    observed: Mutex<Option<Duration>>,
}

impl ObservationOps for TimedResolver {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        *self.observed.lock().expect("capture lock") = Some(deadline.remaining());
        Ok(NativeHandle::null())
    }
}

#[test]
fn locator_anchor_never_falls_back_to_the_public_strict_resolver() {
    let resolver = TimedResolver {
        observed: Mutex::new(None),
    };

    let error = resolver
        .resolve_locator_anchor(&entry(), crate::Deadline::standard().unwrap())
        .err()
        .expect("unsupported exact-path resolution must fail closed");

    assert_eq!(error.code, ErrorCode::PlatformNotSupported);
    assert_eq!(*resolver.observed.lock().expect("capture lock"), None);
}

fn entry() -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.scope.path = Default::default();
    entry
}
