use super::{create, list, load, remove};
use crate::sandbox::sandbox_record::SandboxRecord;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "agent-desktop-sandbox-store-{label}-{}",
            crate::refs::new_snapshot_id()
        ));
        Self(path)
    }

    fn root(&self) -> PathBuf {
        self.0.join("sandboxes")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn record(name: &str, provider: &str, created_at: u64) -> SandboxRecord {
    SandboxRecord {
        name: name.to_string(),
        provider: provider.to_string(),
        created_at,
    }
}

#[test]
fn duplicate_create_fails_and_keeps_the_first_record() {
    let scratch = Scratch::new("duplicate");
    let root = scratch.root();
    create(&root, &record("dev-1", "macos-vm", 1)).unwrap();

    let error = create(&root, &record("dev-1", "macos-vm", 2)).unwrap_err();

    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(
        error
            .to_string()
            .contains("already exists (provider macos-vm)")
    );
    let kept = load(&root, "dev-1").unwrap();
    assert_eq!(kept.provider, "macos-vm");
    assert_eq!(kept.created_at, 1);
}

#[test]
fn concurrent_creates_of_one_name_admit_exactly_one() {
    let scratch = Scratch::new("concurrent");
    let root: Arc<Path> = Arc::from(scratch.root().as_path());
    let threads = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let mut handles = Vec::new();
    for index in 0..threads {
        let root = Arc::clone(&root);
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            create(&root, &record("race", "macos-vm", index as u64)).is_ok()
        }));
    }
    let winners = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|won| *won)
        .count();
    assert_eq!(winners, 1);
}

#[test]
fn load_of_a_missing_name_reports_sandbox_not_found() {
    let scratch = Scratch::new("missing");
    let error = load(&scratch.root(), "ghost").unwrap_err();
    assert_eq!(error.code(), "SANDBOX_NOT_FOUND");
    assert!(
        error
            .suggestion()
            .is_some_and(|s| s.contains("sandbox list"))
    );
}

#[test]
fn names_that_escape_the_root_are_refused() {
    let scratch = Scratch::new("traversal");
    let error = load(&scratch.root(), "../outside").unwrap_err();
    assert_eq!(error.code(), "INVALID_ARGS");
}

#[test]
fn list_sorts_by_name_and_remove_deletes_the_record() {
    let scratch = Scratch::new("list");
    let root = scratch.root();
    create(&root, &record("beta", "macos-vm", 2)).unwrap();
    create(&root, &record("alpha", "macos-vm", 1)).unwrap();

    let names: Vec<String> = list(&root).unwrap().into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["alpha", "beta"]);

    remove(&root, "alpha").unwrap();
    assert_eq!(
        load(&root, "alpha").unwrap_err().code(),
        "SANDBOX_NOT_FOUND"
    );
    assert_eq!(list(&root).unwrap().len(), 1);
}
