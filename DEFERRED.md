# Deferred Beyond v0.1

These items were reviewed during the Phase 6 freeze audit and are intentionally left out of the v0.1 freeze scope.

## Evidence extraction follow-on heuristics

- Current status: broad contract wording without a concrete delta or reference is now rejected from authoritative blocker evidence, and the boundary is covered by tests.
- Why deferred: this pass did not add new fixture-backed false-positive/false-negative evidence strong enough to widen the heuristic boundary without guesswork.
- Follow-up direction: extend fixtures first, then add only the next deterministic evidence rules that measurably improve trust.

## Human-recorded validation rates

- Current status: the checked-in corpus remains a 12-row traceable scaffold and does not yet record the first release-usable `false_residual_rate` or `missed_obvious_blocker_rate`.
- Why deferred: these rates are intentionally a human/manual operational step, not something to synthesize from implementation-only validation or row-count expansion alone.
- Follow-up direction: expand the corpus to a human-confirmed 50-comment sample, record both release metrics in `fixtures/validation/manual_labels_v0.1.yaml`, and then mirror those exact values in `docs/VALIDATION.md`.
