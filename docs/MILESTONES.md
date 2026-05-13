# Milestones

This file tracks product milestones for small, reviewable PRs.

## M0: Product Boundary

Goal:

- make the first PR independently reviewable
- define `review-firewall` as post-review triage, not AI review generation
- record the product boundary in docs and generated artifacts

Included:

- `docs/PRODUCT_BOUNDARY.md`
- README / README_JA positioning updates
- architecture boundary note
- `scan.json.product_boundary`
- scan schema update

Done when:

- `cargo fmt --all --check` passes
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- `cargo test --workspace` passes
- `review-firewall scan` emits `product_boundary`

## M1: v0.1 Deterministic Freeze

Goal:

- finish the local deterministic CLI contract
- keep exactly five commands
- preserve stopless artifact output

Included:

- scan/gate/draft-reply/escalate/report smoke flow
- fixture coverage for blocker and non-blocker Japanese review comments
- config semantics that actually affect gate behavior
- Windows-safe path and run-store behavior

Deferred:

- human-recorded release metrics from the manual validation corpus

## M1.5: v0.1 Trust Hardening

Goal:

- harden the author trust boundary without expanding scope
- separate run success from review-input completeness and author-facing review state
- keep noisy markup and weak evidence from sounding like blockers

Included:

- explicit `data_coverage` and `review_signal`
- terminal/report trust header with `RUN_STATUS`, `DATA_COVERAGE`, `REVIEW_SIGNAL`, and `RESIDUAL_BLOCKERS`
- pre-classification noise normalization for markdown/html badges and links
- evidence-class tracking for blocker extraction
- safer `draft-reply` modes for missing, partial, or non-authoritative analysis

Done when:

- partial review data cannot emit `REVIEW_SIGNAL: CLEAR`
- residual blockers emit `REVIEW_SIGNAL: BLOCKED`
- badge fragments do not appear in `failure_mode`, `evidence`, or `report.md`
- draft replies use `cannot_classify` when gate input is missing, partial, or errored
- docs and emitted artifacts match
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` pass

## M2: v0.1 Release Candidate

Goal:

- prepare binary distribution without expanding product scope

Included:

- release workflow hardening
- release artifact naming
- checksums
- installation notes
- minimal usage examples from a real PR

Non-goals:

- GitHub App integration
- CI required checks
- automatic PR comments
- AI review generation
- review history learning
