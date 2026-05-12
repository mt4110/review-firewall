# Product Boundary

## One-line category

`review-firewall` is a post-review triage firewall for PR conversations.

It is not a code reviewer, PR bot, reviewer reputation system, or CI gate.

## Why this boundary exists

The review-generation space is already crowded with tools that read diffs and produce comments.
Competing there would make this project another review generator.

The sharper product shape is different:

> Existing reviewers produce comments. `review-firewall` classifies, compresses, and routes those comments so the author can act without absorbing the full conversation load.

## Inputs

Allowed inputs for v0.1:

- local git facts
- changed files
- PR metadata via `gh`
- GitHub review comments, including review-level body comments
- GitHub issue comments when available
- CODEOWNERS files in `.github/`, repository root, or `docs/`
- `review-firewall.toml`

Future integrations may import review output from other tools if they preserve this boundary.
They should be treated as review sources, not as engines that this project owns.

## Outputs

Allowed outputs for v0.1:

- `scan.json`
- `gate.json`
- `draft_reply.json`
- `draft_reply.md`
- `escalation.md`
- `report.md`
- terminal `RUN_STATUS:` / `DATA_COVERAGE:` / `REVIEW_SIGNAL:` / `RESIDUAL_BLOCKERS:` lines
- compatibility `STATUS:` / `REASON:` / `NEXT:` lines

These outputs are for the author to inspect and decide from.
They are not automatic PR actions.

## Non-goals

Do not add these to v0.1:

- diff-to-review generation
- LLM-based blocker judgment
- automatic PR comments
- GitHub App installation flow
- CI required checks
- reviewer scoring
- team analytics
- learning from review history
- prompt calibration pipelines
- model orchestration
- auto-fix or auto-merge

## Boundary With Review Generators

Review generators usually do this:

```text
diff/code/context -> review generator -> review comments
```

`review-firewall` does this:

```text
review comments + PR metadata -> deterministic gate -> author-ready artifacts
```

That distinction must stay true even if an optional LLM helper is added later.
LLM use may improve wording, summaries, or ADR titles, but it must not become the core blocker judge.

## Boundary With `local-ai-review`

`local-ai-review` is a local LLM-powered PR review workflow and CLI.
It reviews PR diffs, can post or update PR comments, keeps review history, and includes calibration and learning workflows.

`review-firewall` should not duplicate those responsibilities.

The intended relationship is downstream:

```text
human reviewers / automated review tools / CI summaries
        |
        v
GitHub PR conversation
        |
        v
review-firewall scan -> gate -> draft-reply -> escalate -> report
```

If `local-ai-review` produces a PR comment, `review-firewall` may later classify that comment like any other review input.
It should not run the model, tune the prompt, own review history, or post the generated review.

## Design Test For New Features

Before adding a feature, ask:

1. Does this help the author process existing review conversation?
2. Can it run without LLM-dependent core judgment?
3. Does it avoid posting, scoring, or enforcing decisions on behalf of the author?
4. Does it preserve artifact-first auditability?

If any answer is no, the feature belongs outside v0.1.
