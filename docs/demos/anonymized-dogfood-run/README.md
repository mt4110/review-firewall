# Anonymized Dogfood Demo

This directory checks in one anonymized dogfood run using the current v0.1 artifact contract.

The source run was a local pass on this repository.
Author names, comment ids, and free-form comment bodies were anonymized or trimmed so the example stays reviewable in a public repo.

The `run/` directory mirrors `.review-firewall/run/`:

```text
run/
  latest.json
  20260509T120800.671751000Z/
    scan.json
    gate.json
    draft_reply.json
    draft_reply.md
    escalation.md
    report.md
```

Summary metrics for this run:

```text
397 comments analyzed
45 candidate blockers
11 residual blockers
88.66% comments-to-candidate reduction
97.23% comments-to-residual reduction
```

To keep the demo compact, the checked-in artifacts omit the full comment list and full review-thread list.
They keep the full 45-entry `candidate_blockers` array plus a sampled `classified_comments` slice so the published `397 -> 45 -> 11` demo and the manual-label scaffold stay auditable from the repo.
