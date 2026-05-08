# Deferred Beyond v0.1

These items were reviewed during the Phase 6 freeze audit and are intentionally left out of the v0.1 freeze scope.

## Evidence extraction hardening

- Current status: deterministic and covered by tests, but still somewhat permissive for broad contract wording.
- Why deferred: tightening it further is a quality improvement, not a v0.1 contract fix.
- Follow-up direction: prefer stricter causal markers, repro conditions, or concrete contract deltas before evidence is accepted.

## First-class `reviewDecision` behavior

- Current status: `scan` records top-level `reviewDecision`, but downstream behavior does not treat it as a first-class signal.
- Why deferred: this is useful signal shaping, not a blocker for the v0.1 deterministic CLI contract.
- Follow-up direction: decide whether `reviewDecision` belongs in report/gate summaries or should remain informational only.
