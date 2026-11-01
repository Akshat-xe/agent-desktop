use super::*;

struct RendererLeaseAdapter {
    lease_held: Arc<AtomicBool>,
    window_walks: AtomicU32,
    activations: AtomicU32,
}

struct HeldFlag(Arc<AtomicBool>);

impl Drop for HeldFlag {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl ObservationOps for RendererLeaseAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolve_element_strict(entry, deadline)
    }

    fn list_windows(
        &self,
        _filter: &crate::adapter::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        Ok(vec![crate::WindowInfo {
            id: "w-42".into(),
            title: "Window".into(),
            app: "Fixture".into(),
            pid: crate::ProcessId::new(42),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState::default(),
        }])
    }

    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        let window_walk = matches!(
            root,
            crate::locator::live_locator::ObservationRoot::Window(_)
        );
        if window_walk && self.window_walks.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(AdapterError::renderer_accessibility_activation_required(
                "the renderer publishes no accessibility tree yet",
            ));
        }
        crate::adapter::saved_ref_tree(root, "button", Some("Target"))
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 100.0,
            y: 200.0,
            width: 20.0,
            height: 10.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::ReachesTarget)
    }
}

impl ActionOps for RendererLeaseAdapter {}

impl InputOps for RendererLeaseAdapter {}

impl SystemOps for RendererLeaseAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        assert!(
            !self.lease_held.swap(true, Ordering::SeqCst),
            "the process lease is not reentrant"
        );
        crate::InteractionLease::guarded(deadline, HeldFlag(Arc::clone(&self.lease_held)))
    }

    fn activate_renderer_accessibility(
        &self,
        _process: crate::ProcessIdentity,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        assert!(self.lease_held.load(Ordering::SeqCst));
        self.activations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn a_final_check_resolves_under_the_lease_the_gesture_holds() {
    let _guard = HomeGuard::new();
    let snapshot_id = point_ref_snapshot();
    let adapter = RendererLeaseAdapter {
        lease_held: Arc::new(AtomicBool::new(false)),
        window_walks: AtomicU32::new(0),
        activations: AtomicU32::new(0),
    };
    let deadline = crate::Deadline::after(5_000).unwrap();
    let lease = adapter.acquire_interaction_lease(deadline).unwrap();

    let resolved = resolve_point_under_lease(
        PointResolveAttempt {
            args: point_args(&snapshot_id),
            stability: None,
            allow_scroll: false,
        },
        deadline,
        &lease,
        &adapter,
        &crate::CommandContext::default(),
    )
    .expect("a check that forbids scrolling still resolves under the held lease");

    assert_eq!((resolved.point.x, resolved.point.y), (110.0, 205.0));
    assert_eq!(adapter.activations.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.window_walks.load(Ordering::SeqCst), 2);
}
