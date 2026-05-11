# Trust Hardening Plan

## Goal

Ship `review-firewall` v0.1 as an author-side review signal compressor with a hardened trust boundary.

This phase does not add product scope.
It clarifies whether a run completed, whether review inputs were fully observed, and whether the author-facing review signal is actually clear.

## Context

Relevant source documents:

- `docs/ARCHITECTURE.md`
- `docs/ARTIFACT_SCHEMA.md`
- `docs/PRODUCT_BOUNDARY.md`
- `.private_docs/review_firewall_strategy_docs/01_unshakable_core.md`
- `.private_docs/review_firewall_strategy_docs/04_loophole_register_and_fixes.md`
- `.private_docs/review_firewall_strategy_docs/05_v01_trust_hardening_plan.md`
- `.private_docs/review_firewall_strategy_docs/06_artifact_and_status_contract.md`

Current risk:

- `STATUS: OK` can be misread as “merge-safe”
- partial GitHub data can look clearer than it is
- markdown / badge noise can leak into blocker extraction
- reply drafts can sound more authoritative than the analysis deserves

## Constraints

- keep exactly five commands:
  - `scan`
  - `gate`
  - `draft-reply`
  - `escalate`
  - `report`
- keep the core deterministic and Rust-only
- do not add automatic PR posting, CI enforcement, or review generation
- preserve stopless behavior and keep `STATUS:` available in terminal output
- treat `changed path` alone as non-evidence
- keep CODEOWNERS ownership advisory, not authority

## Implementation Scope

### 1. Status split

Add explicit author-facing trust signals:

- `status` as the run/artifact generation status
- `data_coverage` as review-input completeness
- `review_signal` as `BLOCKED | CLEAR | UNKNOWN`

Terminal and report headers should show:

```text
RUN_STATUS: ...
DATA_COVERAGE: ...
REVIEW_SIGNAL: ...
RESIDUAL_BLOCKERS: ...
STATUS: ...
```

`STATUS:` remains as a compatibility alias for stopless workflows.

### 2. Noise normalization

Normalize comment text before classification:

- strip badge/image HTML and markdown image fragments
- reduce markdown links to visible text
- keep inline code only as intentional evidence references
- prevent badge fragments such as `style=flat` from entering blocker fields

### 3. Evidence classes

Track concrete evidence shape for blocker extraction:

- `causal_runtime_failure`
- `contract_delta`
- `repro_condition`
- `security_condition`
- `ci_test_failure`
- `concrete_reference`
- `keyword_only`
- `path_only`
- `noise_only`

Residual blockers must never be emitted from:

- `keyword_only`
- `path_only`
- `noise_only`

### 4. Draft reply safety

Support safe reply modes:

- `accept`
- `ask_for_evidence`
- `ask_for_scope`
- `move_to_adr`
- `move_to_rfc`
- `needs_human_judgment`
- `cannot_classify`

If gate analysis is missing, partial, or errored, the draft must stay non-authoritative and avoid merge-safety claims.

### 5. Schema and report sync

Keep docs and emitted artifacts aligned.
`report.md` should surface the trust header clearly before the human summary.

## Done When

- docs reflect the split between run status, data coverage, and review signal
- partial data cannot produce `REVIEW_SIGNAL: CLEAR`
- residual blockers imply `REVIEW_SIGNAL: BLOCKED`
- badge / markdown noise does not appear in `failure_mode`, `evidence`, or `report.md`
- draft reply uses `cannot_classify` on missing or non-authoritative gate input
- `cargo fmt --all` passes
- `cargo clippy --workspace --all-targets -- -D warnings` passes
- `cargo test --workspace` passes
- `scan`, `gate`, `draft-reply`, `escalate`, and `report` still write artifacts on stopless paths
