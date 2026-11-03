//! Tool calls in flight, keyed by JSON-RPC request id, so `notifications/cancelled` can stop one.

use serde_json::Value;
use std::collections::{HashMap, hash_map::Entry};
use std::io;
use std::process::{Child, ChildStderr, ChildStdout, Command};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Output pipes of a started call.
pub(crate) type Pipes = (Option<ChildStdout>, Option<ChildStderr>);

/// Each admitted request id, with the child process its call started once it has started one.
#[derive(Default)]
pub(crate) struct Calls(Mutex<HashMap<String, Option<Child>>>);

impl Calls {
    /// Admits `id` before its call starts; false when a call with that id is already in flight.
    pub(crate) fn admit(&self, id: &Value) -> bool {
        match self.lock().entry(id.to_string()) {
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                entry.insert(None);
                true
            }
        }
    }

    /// Spawns `command` for `id` and returns its pipes, or `None` when `id` is not admitted or was cancelled.
    /// The registry stays locked while the child spawns, so a cancel lands either before it, and nothing
    /// starts, or after it, and the child is killed.
    pub(crate) fn start(&self, id: &Value, command: &mut Command) -> Option<io::Result<Pipes>> {
        let mut calls = self.lock();
        let slot = calls.get_mut(&id.to_string())?;
        Some(command.spawn().map(|mut child| {
            let pipes = (child.stdout.take(), child.stderr.take());
            *slot = Some(child);
            pipes
        }))
    }

    /// Takes back the child of `id` after its output closed, or `None` when `id` was cancelled.
    pub(crate) fn take(&self, id: &Value) -> Option<Child> {
        self.lock().get_mut(&id.to_string()).and_then(Option::take)
    }

    /// Cancels `id` and kills its child if it has one; an unknown id is ignored.
    pub(crate) fn cancel(&self, id: &Value) {
        let child = self.lock().remove(&id.to_string()).flatten();
        if let Some(mut child) = child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Releases `id` once its call has returned; false when it was cancelled, so no response is due.
    pub(crate) fn finish(&self, id: &Value) -> bool {
        self.lock().remove(&id.to_string()).is_some()
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Option<Child>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "calls_tests.rs"]
mod tests;
