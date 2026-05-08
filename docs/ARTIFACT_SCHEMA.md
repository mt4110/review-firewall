# Artifact Schema

This document defines the minimum stable artifact contracts for v0.1.

These contracts are intentionally simple.
If a field is added, it must be backward compatible or versioned.

## Run store layout

```text
.review-firewall/
  run/
    latest.json
    <timestamp>/
      scan.json
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
- `labels`
- `comments`
- `issue_comments`
- `review_decisions`
- `partial_sources`

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

## `gate.json`

Minimum shape:

```json
{
  "status": "OK",
  "comments_analyzed": 17,
  "residual_blockers": [
    {
      "comment_id": "12",
      "concern": "correctness",
      "failure_mode": "partial status may break response contract",
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

## `draft_reply.json`

Minimum shape:

```json
{
  "status": "OK",
  "reply_type": "accept",
  "target_comment_id": "12",
  "body": "Thanks. I agree this is a correctness issue in this PR.\nI will address it here by updating the contract handling."
}
```

Reply types:

- `accept`
- `decline`
- `move`

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

Must include three sections:

1. residual blockers
2. PM summary
3. author action list

Suggested shape:

```md
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
- `status` meanings are stable
- `latest.json` replaces symlink-based latest pointers
