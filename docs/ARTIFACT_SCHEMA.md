# Artifact Schema

This document defines the minimum stable artifact contracts for v0.1.

These contracts are intentionally simple.
If a field is added, it must be backward compatible or versioned.

The JSON Schema files under `schemas/` describe the current emitted artifact shape.
Rust deserialization may remain backward compatible with selected legacy fields so older local artifacts can still be read during upgrades.

## Run store layout

```text
.review-firewall/
  run/
    latest.json
    <timestamp>/
      scan.json
      source_coverage.json
      gate.json
      draft_reply.json
      draft_reply.md
      escalation.md
      report.md
```

## Timestamp format

Use UTC compact form with nanosecond precision:

```text
YYYYMMDDTHHMMSS.NNNNNNNNNZ
```

When a run directory collides on a coarse clock, append a two-digit numeric suffix:

```text
YYYYMMDDTHHMMSS.NNNNNNNNNZ-01
```

Example:

```text
20260328T203500.123456789Z
```

## Common fields

### `status`

Allowed values:

- `OK`
- `PARTIAL`
- `ERROR`

This is the run/artifact generation status.

### `data_coverage`

Allowed values:

- `FULL`
- `PARTIAL`
- `FAILED`

This tracks whether review inputs were fully observed.

### `review_signal`

Allowed values:

- `BLOCKED`
- `CLEAR`
- `UNKNOWN`

This is the author-facing review state.
It must be derived independently from `status`.

### `reason`

Human-readable explanation for partial/error states.

## `latest.json`

```json
{
  "timestamp": "20260328T203500Z"
}
```

## `scan.json`

Minimum shape:

```json
{
  "status": "OK",
  "data_coverage": "FULL",
  "review_signal": "UNKNOWN",
  "pr": {
    "number": 142,
    "title": "Refactor response handling"
  },
  "files_changed": 8,
  "review_comments": 17,
  "threads": 6,
  "codeowners_found": true,
  "policy_found": true,
  "product_boundary": {
    "category": "post_review_triage_firewall",
    "consumes_existing_review_comments": true,
    "generates_ai_reviews": false,
    "posts_pr_comments": false,
    "uses_llm_for_core_judgment": false
  }
}
```

Recommended additional fields:

- `repo_root`
- `branch`
- `base_branch`
- `head_oid`
- `labels`
- `comments`
- `issue_comments`
- `review_decisions`
- `partial_sources`
- `scan_partial` as a compatibility flag

`scan.json.pr.review_decisions` carries the current top-level GitHub review state when available, with the latest effective state derived from review history used only as a fallback when that top-level field is absent.
It is downstream-visible, but it remains informational input rather than blocker evidence.

`scan.json.partial_sources` remains a lightweight compatibility hint.
The authoritative per-source ingestion view lives in `source_coverage.json`.

`product_boundary` records the product-category contract for auditability:

```json
{
  "category": "post_review_triage_firewall",
  "consumes_existing_review_comments": true,
  "generates_ai_reviews": false,
  "posts_pr_comments": false,
  "uses_llm_for_core_judgment": false
}
```

## `source_coverage.json`

Purpose:

- make missing review inputs explicit
- normalize scan-time ingestion failures into stable reason classes
- derive `data_coverage` from required sources without hiding partial local advisory reads

Minimum shape:

```json
{
  "status": "PARTIAL",
  "data_coverage": "PARTIAL",
  "review_signal": "UNKNOWN",
  "sources": [
    {
      "name": "pr_metadata",
      "required": true,
      "status": "FULL",
      "items_seen": 1
    },
    {
      "name": "review_comments",
      "required": true,
      "status": "PARTIAL",
      "items_seen": 100,
      "failure_reason": "pagination_partial",
      "detail": "GitHub pagination stopped after page 1 while fetching review comments.",
      "retry_hint": "Rerun review-firewall scan after checking gh auth, rate limits, or connectivity."
    }
  ]
}
```

Source status values:

- `FULL`
- `PARTIAL`
- `FAILED`
- `SKIPPED`

Stable failure-reason values used by scan-time ingestion:

- `gh_missing`
- `gh_not_authenticated`
- `gh_rate_limited`
- `gh_permission_denied`
- `pr_not_found`
- `repository_identity_unknown`
- `pagination_partial`
- `json_parse_error`
- `network_error`
- `local_git_unavailable`
- `head_oid_mismatch`
- `unsupported_remote`

`data_coverage` is derived only from required sources:

- any required `FAILED` or `SKIPPED` source => `FAILED`
- else any required `PARTIAL` source => `PARTIAL`
- else => `FULL`

## `gate.json`

Minimum shape:

```json
{
  "status": "OK",
  "data_coverage": "FULL",
  "review_signal": "BLOCKED",
  "comments_analyzed": 17,
  "residual_blockers": [
    {
      "comment_id": "12",
      "concern": "correctness",
      "failure_mode": "partial status may break response contract",
      "evidence_class": "contract_delta",
      "evidence": ["response contract changes when status=partial"],
      "owner_match": true,
      "ownership_scope": "exact",
      "advisory_weight": "high"
    }
  ],
  "counts": {
    "questions": 4,
    "suggestions": 5,
    "nits": 4,
    "praise": 2
  }
}
```

Recommended additional fields:

- `candidate_blockers`
- `downgraded_comments`
- `duplicates_collapsed`
- `warnings`
- `config_snapshot`
- `classified_comments`
- `escalation_candidates`
- `review_decision_summary`

When present, `review_decision_summary` should look like:

```json
{
  "states": ["CHANGES_REQUESTED"],
  "changes_requested": true,
  "approved": false,
  "review_required": false,
  "informational_only": true
}
```

This summary is first-class reviewer-state context for the author, but it must never override `review_signal` or create residual blockers on its own.

## `draft_reply.json`

Minimum shape:

```json
{
  "status": "OK",
  "data_coverage": "FULL",
  "review_signal": "BLOCKED",
  "reply_type": "accept",
  "target_comment_id": "12",
  "body": "Thanks. I agree this is a correctness issue in this PR.\nI will address it here by updating the contract handling."
}
```

Reply types:

- `accept`
- `ask_for_evidence`
- `ask_for_scope`
- `move_to_adr`
- `move_to_rfc`
- `needs_human_judgment`
- `cannot_classify`

## `draft_reply.md`

Markdown rendering of the selected replies.
Keep each reply concise.
Default target: 3 lines or fewer per reply.

## `escalation.md`

Minimum shape:

```md
# ADR Candidate

## Title
Response contract handling for partial status

## Why this was escalated
This discussion exceeded PR-local review scope.

## Position A
Keep current response shape and handle partial via metadata.

## Position B
Change response schema to represent partial explicitly.

## Decision needed
- Is this a PR blocker?
- Which contract becomes source of truth?
- Can current PR merge before this is decided?

## Related PR
#142
```

## `report.md`

The header should begin with:

```md
RUN_STATUS: OK
DATA_COVERAGE: FULL
REVIEW_SIGNAL: BLOCKED
RESIDUAL_BLOCKERS: 1
STATUS: OK
```

Must include three sections:

1. residual blockers
2. PM summary
3. author action list

When top-level review-decision state is available, `report.md` may also surface it as informational reviewer state.

Suggested shape:

```md
RUN_STATUS: OK
DATA_COVERAGE: FULL
REVIEW_SIGNAL: BLOCKED
RESIDUAL_BLOCKERS: 1
STATUS: OK

# Review Firewall Report

## Residual blockers
- correctness: partial status may break response contract

## PM summary
Residual blockers: 1
Impact: current response contract may break partial-status clients
Action: decide contract handling in this PR or move schema redesign to ADR

## Author action list
1. Reply accept to comment #12
2. Reply move-to-ADR to thread #3
3. Update response contract test
```

## Compatibility rules

- New fields may be added, but existing fields must not change meaning silently
- `status` remains the run-status field in JSON artifacts
- `RUN_STATUS:` in terminal/report output is the human-facing header for that same status
- `latest.json` replaces symlink-based latest pointers
