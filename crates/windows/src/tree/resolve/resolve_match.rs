use agent_desktop_core::{
    AdapterError, LocatorEvidence, LocatorField, RefEntry,
    refs::ref_identity::{has_stable_text_identity, identity_match, stable_text_match},
};
use serde_json::json;

use super::resolve_search::NodeAdmission;
use crate::tree::element::UIAElement;

/// A matched live element plus the tie-break hash read off its evidence.
#[derive(Clone)]
pub(crate) struct Candidate {
    pub(crate) element: UIAElement,
    pub(crate) bounds_hash: Option<u64>,
}

/// The three-state outcome the composed rule demands: a candidate either
/// matched, was refuted, or **could not be read** - and an unreadable
/// candidate makes the attempt incomplete-and-retryable rather than silently
/// non-matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateOutcome {
    /// The candidate is the stored element.
    Matched,
    /// A read field contradicts the stored evidence; the candidate is not it.
    Refuted,
    /// A field that decides the question could not be read. Never a `NoMatch`.
    Incomplete,
}

/// The composed rule over core's verdicts.
///
/// `identity_match` settles the id tier for an id-bearing ref, and by itself
/// would settle on an id hit without consulting text - the exact behaviour
/// A7-3 measured as silently wrong on Windows (Explorer re-resolving 5 of 29
/// index-keyed refs onto a different element). The corroboration tier runs
/// only when the stored ref carries a stable text identity
/// (`has_stable_text_identity`): an id `Match` + stable-text `NoMatch` is
/// refuted, and a `NoMatch` never collapses an `Unknown`. The tier keys the
/// incomplete flag on the verdict itself: an `IdentityMatch::Unknown`, which
/// core produces when a decisive field reads `LocatorField::Unknown` (a read
/// that failed), becomes `Incomplete`. A structurally absent field is not
/// `Unknown` and settles `NoMatch` instead, because `match_primary_identity`
/// returns `NoMatch` when both the primary and fallback fields are `Absent`. An
/// id `Match` on a ref with no stored text identity (an icon-only button)
/// resolves without corroboration, preserving the previous behaviour. An id-less ref
/// is routed through `identity_match`, which performs the text match itself.
pub(crate) fn candidate_outcome(entry: &RefEntry, evidence: &LocatorEvidence) -> CandidateOutcome {
    let id_tier = identity_match(
        entry,
        &evidence.name,
        &evidence.value,
        &evidence.description,
        &evidence.identifiers,
    );
    if entry.identity.native_id.is_some()
        && id_tier == agent_desktop_core::IdentityMatch::Match
        && has_stable_text_identity(entry)
    {
        return outcome_of(stable_text_match(
            entry,
            &evidence.name,
            &evidence.value,
            &evidence.description,
        ));
    }
    outcome_of(id_tier)
}

/// The one mapping from a core verdict to this resolver's outcome. Written
/// once so the two tiers cannot drift apart: an unreadable field is always
/// `Incomplete`, never a refutation.
fn outcome_of(verdict: agent_desktop_core::IdentityMatch) -> CandidateOutcome {
    match verdict {
        agent_desktop_core::IdentityMatch::Match => CandidateOutcome::Matched,
        agent_desktop_core::IdentityMatch::NoMatch => CandidateOutcome::Refuted,
        agent_desktop_core::IdentityMatch::Unknown => CandidateOutcome::Incomplete,
    }
}

/// Derives the tie-break hash from the walk-shaped evidence slot.
pub(crate) fn bounds_hash_of(evidence: &LocatorEvidence) -> Option<u64> {
    match evidence.ref_evidence.bounds {
        LocatorField::Known(rect) => rect.bounds_hash(),
        LocatorField::Absent | LocatorField::Unknown => None,
    }
}

/// What the 2+-candidate branch decided.
pub(crate) enum Selection {
    /// Exactly one candidate matched the stored bounds hash; its index in the
    /// searched candidate list.
    Resolved(usize),
    /// No stored hash, no candidate matched it, or more than one did.
    Ambiguous,
}

/// Breaks a tie among several evidence-equal candidates by bounds hash.
///
/// Pure over the hashes alone, not the elements, so the decision is testable
/// without a live COM element in scope. The bounds hash never refutes an
/// exact match; it only ever picks among several - a stored ref with no hash
/// (bounds failed at capture, or hidden by the requester) cannot be
/// disambiguated this way, so it stays ambiguous rather than guessing.
pub(crate) fn select_by_bounds_hash(
    candidate_hashes: &[Option<u64>],
    expected_hash: Option<u64>,
) -> Selection {
    let Some(expected_hash) = expected_hash else {
        return Selection::Ambiguous;
    };
    let mut sole = None;
    let mut matches = 0;
    for (index, hash) in candidate_hashes.iter().enumerate() {
        if *hash == Some(expected_hash) {
            matches += 1;
            sole = Some(index);
        }
    }
    match (matches, sole) {
        (1, Some(index)) => Selection::Resolved(index),
        _ => Selection::Ambiguous,
    }
}

/// The role gate every admission rule shares: `None` when the candidate's role
/// matches the stored one, so the identity rule decides, and otherwise the
/// gate's own answer. A role that could not be read leaves the region unread,
/// while a role that read back as something else is a real refusal.
///
/// One copy rather than one per admission rule, so the positional search and
/// the identity-uniqueness search can never accept different elements on the
/// strength of the role alone.
pub(crate) fn role_admission(
    entry: &RefEntry,
    evidence: &LocatorEvidence,
) -> Option<NodeAdmission> {
    if evidence
        .role
        .known()
        .is_some_and(|role| role == &entry.identity.role)
    {
        return None;
    }
    if evidence.role.is_unknown() {
        return Some(NodeAdmission::Unread);
    }
    Some(NodeAdmission::Reject)
}

/// The one mapping from a core verdict to the resolver's admission. Written
/// once so no admission rule can drift apart from another: an unreadable field
/// is always `Unread`, never a refutation.
pub(crate) fn admitted_outcome(outcome: CandidateOutcome) -> NodeAdmission {
    match outcome {
        CandidateOutcome::Matched => NodeAdmission::Collect,
        CandidateOutcome::Incomplete => NodeAdmission::Unread,
        CandidateOutcome::Refuted => NodeAdmission::Reject,
    }
}

/// Whether an already-collected match count settles `AMBIGUOUS_TARGET` on
/// its own, with nothing left to read that could change the answer.
///
/// Despite the name, nothing here stops `search_under`'s own walk early -
/// Windows's search always collects the whole depth-bounded subtree, and
/// this predicate is consulted only after collection finishes, inside
/// `classify_search`. Two or more matches with no stored hash to
/// disambiguate is conclusively ambiguous already: nothing left unread could
/// turn it into anything other than `AMBIGUOUS_TARGET`, so an incomplete
/// region elsewhere in the tree must not withhold that verdict. Mirrors
/// macOS's `should_stop_collecting`
/// (`crates/macos/src/tree/resolve/resolve_search.rs:374-376`), which does stop its
/// walk early on this signal - Windows only reuses its name and its
/// classification rule, not its early exit.
pub(crate) fn should_stop_collecting(match_count: usize, entry: &RefEntry) -> bool {
    match_count > 1 && entry.geometry.bounds_hash.is_none()
}

pub(crate) fn stale_ref_error(_entry: &RefEntry) -> AdapterError {
    let error = AdapterError::stale_ref_because("Stored ref does not match any live element");
    let default_retryable = error.permits_retry_by_default();
    error.with_details(json!({
        "kind": "resolve_no_candidate",
        "complete": true,
        "retryable": default_retryable,
    }))
}

/// The terminal twin of [`stale_ref_error`]: the ref's owning process has
/// itself exited, so no re-attempt within this deadline can ever find a live
/// candidate. Stamped `retryable: false` so core's poll loop settles on the
/// first attempt instead of re-running resolution to the outer deadline and
/// reporting a bare `TIMEOUT` that discards the diagnosis (A14-4 measured
/// ~5s at the default budget for a defect this stamp settles in one pass).
/// `ErrorCode::StaleRef` on a `not_delivered` disposition already carries the
/// `refresh_snapshot_then_retry_original` recovery hint (`output.rs`'s
/// `recovery_for_code`), so nothing further needs attaching here.
#[cfg(target_os = "windows")]
pub(crate) fn owning_process_exited_error(_entry: &RefEntry) -> AdapterError {
    AdapterError::stale_ref_because("Stored ref's owning process has exited").with_details(json!({
        "kind": "resolve_owning_process_exited",
        "complete": true,
        "retryable": false,
    }))
}

/// The settled `AMBIGUOUS_TARGET`, stamped on both retry axes like every
/// other resolver verdict.
///
/// `complete` is true because every path that reaches here has a conclusive
/// answer: `classify_search` withholds the verdict as `Incomplete` whenever an
/// unread region could still change it, and the one case it lets through with
/// an unread region (`should_stop_collecting`: two or more matches and no
/// stored bounds hash to separate them) cannot be resolved by any further
/// candidate, only made more ambiguous.
///
/// `retryable` is false because nothing about a re-attempt differs: the stored
/// evidence is fixed, and the search that produced several equally-matching
/// candidates is deterministic over the same tree. The stamp keeps that
/// verdict out of core's poll loop, which retries a resolution failure only
/// when it is explicitly retryable.
pub(crate) fn ambiguous_target_error(_entry: &RefEntry, count: usize) -> AdapterError {
    AdapterError::ambiguous_target("Multiple live elements match the stored identity")
        .with_suggestion("Take a fresh snapshot and use its new ref")
        .with_details(json!({
            "kind": "resolve_ambiguous",
            "candidate_count": count,
            "complete": true,
            "retryable": false,
        }))
}

#[cfg(test)]
#[path = "resolve_match_tests.rs"]
mod tests;
