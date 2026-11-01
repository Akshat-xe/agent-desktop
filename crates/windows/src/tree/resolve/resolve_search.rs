//! The stored-path landing family: the eligibility predicates the locator
//! anchor gates on, the geometry tier it promotes with, and the bounded child
//! enumeration it descends through (in `descent`).
//!
//! Split from `resolve.rs` so that neither file crosses the 400-line cap.
//! Everything here mirrors a shape the macOS adapter already settled: the
//! geometry promotion predicate `provisional_geometry_candidate`
//! (`crates/macos/src/tree/resolve/resolve_search.rs`) - with one Windows-measured
//! addition, that the stored hash must come from a positive-area rectangle
//! (A17-7).

use agent_desktop_core::{
    Deadline, LocatorEvidence, RefEntry, refs::ref_identity::has_meaningful_identity,
};

use super::resolve_match::bounds_hash_of;
use crate::tree::walk::walker::{DEFAULT_MAX_SIBLINGS, WalkBudget};

#[path = "resolve_descent.rs"]
pub(crate) mod descent;

/// The resolve-scoped depth cap.
///
/// Independently bounded from the walk's own ceiling, mirroring macOS's
/// `MAX_RESOLVE_DEPTH` (`crates/macos/src/tree/resolve/mod.rs:17`). Electron
/// elements commonly sit at depth 25+, so the cap is the resolution bound
/// rather than the walk bound.
pub(crate) const MAX_RESOLVE_DEPTH: u8 = 50;

/// The budget a resolution attempt enumerates under.
///
/// The sibling bound is the observation walk's own `DEFAULT_MAX_SIBLINGS`
/// rather than a resolve-scoped number, and that is load-bearing rather than
/// incidental. `read_children` reports a cap-hit as a whole child list, so a
/// truncation here settles `STALE_REF` instead of retrying; that verdict is
/// only honest while the resolver reaches at least as far as the walk that
/// issued the ref, which presents no cap-cut list as whole and therefore
/// allocates no ref past its own bound. A lower bound here would convert
/// reachable refs into settled misses with nothing else refuting them.
pub(crate) fn resolve_walk_budget(deadline: Deadline) -> WalkBudget {
    WalkBudget::new(MAX_RESOLVE_DEPTH, deadline)
        .with_max_raw_depth(MAX_RESOLVE_DEPTH)
        .with_max_siblings(DEFAULT_MAX_SIBLINGS)
}

/// The incomplete-and-retryable answer: a candidate that could not be read is
/// not a non-match. Core owns the payload
/// (`agent_desktop_core::refs::resolve_errors::identity_unknown_error`) because it
/// is byte-identical to macOS's `identity_unknown` - a `complete: false,
/// retryable: true` stamp so the caller's loop retries it.
pub(crate) use agent_desktop_core::refs::resolve_errors::identity_unknown_error;

/// Whether the stored path is scoped to the window root rather than to a
/// drill-down ancestor: no `root_ref` at all, or an absolute path that
/// overrides one. Shared by every eligibility gate below, so
/// window-rootedness is decided in exactly one place.
pub(crate) fn window_rooted(entry: &RefEntry) -> bool {
    entry.scope.root_ref.is_none() || entry.scope.path_is_absolute
}

/// Whether the stored ref is locatable by the anchor at all: window-rooted,
/// and not [`entry_is_unverifiable`] - an id, stable text, or a positive-area
/// bounds hash eligible for the geometry tier
/// ([`provisional_geometry_candidate`] is the exact predicate that tier
/// promotes on, reused rather than re-derived).
///
/// This is the anchor's entire eligibility gate. A bare
/// `RefEntry::geometry.bounds_hash` is not this signal: a zero-extent
/// rectangle still hashes (`Rect::bounds_hash` only rejects an invalid
/// rectangle, not an empty one), so a hash-only check would pass an id-less,
/// text-less entry the geometry tier can never actually promote, land the
/// path, and only then discover it cannot verify - burning the retry budget
/// on a landing that was never eligible.
pub(crate) fn entry_is_locatable(entry: &RefEntry) -> bool {
    window_rooted(entry) && !entry_is_unverifiable(entry)
}

/// The geometry promotion eligibility predicate.
///
/// Mirrors macOS's `provisional_geometry_candidate` with one
/// Windows-measured addition: the stored bounds hash must come from a
/// positive-area rectangle (A17-7 - offscreen and virtualized elements
/// collapse to shared zero-extent bounds that are structurally non-unique),
/// and the stored bounds must agree. A ref with no meaningful text identity
/// and a zero-extent stored hash is unresolvable by design and never promotes.
pub(crate) fn provisional_geometry_candidate(entry: &RefEntry) -> bool {
    entry.geometry.bounds_hash.is_some()
        && entry
            .geometry
            .bounds
            .is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0)
        && !has_meaningful_identity(entry)
}

/// Whether a candidate's live bounds hash matches the stored one under the
/// promotion's eligibility rules - the unique-geometry-match that promotes an
/// otherwise-unreadable candidate to resolved.
pub(crate) fn geometry_matches(entry: &RefEntry, evidence: &LocatorEvidence) -> bool {
    provisional_geometry_candidate(entry) && bounds_hash_of(evidence) == entry.geometry.bounds_hash
}

/// Whether the stored ref carries nothing any resolution tier could ever
/// verify a live candidate against: no id, no stable text, and no
/// positive-area bounds hash for the geometry tier to promote on.
///
/// Meant to be checked once over the entry before the walk runs, because no
/// amount of successful reading changes the answer for this class: with no
/// stored name, value or description, core's `stable_text_match` has no
/// `expected` text to equality-match against, so it can only ever fall
/// through to `empty_identity_match` over the live candidate's own fields -
/// which resolves `NoMatch` the instant any of them reads back `Known`, and
/// `Unknown` only when none of them do. `IdentityMatch::Match` is therefore
/// unreachable for this class no matter what the walk reads, so a resolver
/// that walked the tree would spend a full attempt reaching a verdict the
/// entry alone already settles.
pub(crate) fn entry_is_unverifiable(entry: &RefEntry) -> bool {
    !has_meaningful_identity(entry) && !provisional_geometry_candidate(entry)
}

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_search_tests.rs"]
mod tests;
