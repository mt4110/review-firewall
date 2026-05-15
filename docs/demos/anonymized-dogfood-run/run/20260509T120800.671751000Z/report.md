RUN_STATUS: OK
DATA_COVERAGE: FULL
REVIEW_SIGNAL: BLOCKED
RESIDUAL_BLOCKERS: 11
STATUS: OK
REVIEW_DECISIONS: COMMENTED (informational only)

# Review Firewall Report

## Residual blockers
- api [contract_delta]: Report status can hide downstream partial or error artifacts when it is sourced from gate alone.
- correctness [concrete_reference]: Badge fragments should be stripped before failure-mode matching or noise can survive into blocker summaries.
- security [security_condition]: GitHub metadata fetch failures should degrade data coverage to PARTIAL instead of sounding like fatal review truth.
- api [contract_delta]: Missing repository identity can silently drop changed-file and PR-comment fetches, which hides review evidence from gate.
- api [contract_delta]: If draft_reply.json is missing, report can look merely partial and mask a real upstream error path.
- security [repro_condition]: A stale local base diff can add files that are not part of the actual PR against the remote base.
- security [security_condition]: Security terms such as xss, csrf, and sql injection should not be filtered out as metalinguistic noise.
- correctness [contract_delta]: A missing or partial gate analysis must not produce a reply draft that sounds safe to post.
- api [concrete_reference]: Synthetic root ids can merge or split threads incorrectly and skew roundtrip counts.
- operability [repro_condition]: Malformed ownerless CODEOWNERS lines can distort advisory weight for affected paths.
- security [repro_condition]: When GitHub is unavailable, fallback diffing must not assume the local base branch matches the remote PR base.

## PM summary
Residual blockers: 11
Reviewer state: COMMENTED (informational only)
Impact: Report status can hide downstream partial or error artifacts when it is sourced from gate alone.
Action: decide whether to fix in this PR or move the broader design issue out of band

## Author action list
1. Address blocker #demo-r001: Report status can hide downstream partial or error artifacts when it is sourced from gate alone.
2. Address blocker #demo-r002: Badge fragments should be stripped before failure-mode matching or noise can survive into blocker summaries.
3. Address blocker #demo-r003: GitHub metadata fetch failures should degrade data coverage to PARTIAL instead of sounding like fatal review truth.
4. Use the accept reply draft: Thanks. I agree this is an API contract issue in this PR. / I will merge report status across downstream artifacts before asking for re-review.

## Source coverage
Review-input coverage: FULL
Incomplete required sources: 0
- Repo root: FULL (required, 1 seen)
- Current branch: FULL (optional, 1 seen)
- Config: FULL (optional, 1 seen)
- CODEOWNERS: FULL (optional, 1 seen)
- PR metadata: FULL (required, 1 seen)
- Changed files: FULL (required, 76 seen)
- Review comments: FULL (required, 223 seen)
- Review body comments: FULL (required, 51 seen)
- Issue comments: FULL (required, 123 seen)
- Review decision: FULL (optional, 1 seen)
