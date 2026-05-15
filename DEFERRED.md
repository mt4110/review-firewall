# Deferred Beyond v0.1

These items were reviewed during the Phase 6 freeze audit and are intentionally left out of the v0.1 freeze scope.

Historical note:

- The narrow evidence follow-on from the design docs is no longer part of the current deferred set.
- The checked-in demo had already captured a traceable metalinguistic false-positive family around `failure-mode extractor` / `failure-mode matching` wording.
- That narrow deterministic follow-up is now handled in code and tests without widening v0.1 scope beyond the existing evidence-hardening contract.

## Human-recorded validation rates

- Current status: the checked-in corpus remains a 12-row traceable scaffold and does not yet record the first release-usable `false_residual_rate` or `missed_obvious_blocker_rate`.
- Why deferred: these rates are intentionally a human/manual operational step, not something to synthesize from implementation-only validation or row-count expansion alone.
- Follow-up direction: expand the corpus to a human-confirmed 50-comment sample, record both release metrics in `fixtures/validation/manual_labels_v0.1.yaml`, and then mirror those exact values in `docs/VALIDATION.md`.
