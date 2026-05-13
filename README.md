# review-firewall

**Review the review. Protect the author.**

`review-firewall` is an author-side review signal compressor for noisy PR conversations.
It converts comments that already exist into typed, auditable signals the author can act on without absorbing the full thread.

It does **not** generate AI reviews.
It does **not** replace human review.
It does **not** decide merge safety.
It does **not** post PR comments automatically.

It does one thing:

> **It reviews the review process itself.**

## Why this exists

In many teams, PR threads mix together four different things:

- real breakage risk
- design assumptions
- local conventions
- personal preference

When these are mixed, review becomes political noise.
The author pays first.

`review-firewall` exists to separate those concerns and turn vague pressure into typed, inspectable signals.

## Positioning

`review-firewall` sits after review comments already exist, on the author side of the workflow.

It can consume comments from humans, automated review tools, CI summaries, or local review workflows, but it does not create those reviews itself. Its job is to decide what is actually actionable for the author: which comments are evidence-backed blockers, which should become questions or suggestions, and which design debates should leave the PR.

See [Product Boundary](docs/PRODUCT_BOUNDARY.md) for the non-overlap contract.

## Example

One anonymized dogfood run on this repo compressed:

```text
397 comments analyzed
45 candidate blockers
11 residual blockers
88.66% comments-to-candidate reduction
97.23% comments-to-residual reduction
```

That is the product: not more review comments, but less review pressure and clearer next action.

See the checked-in [anonymized dogfood demo](docs/demos/anonymized-dogfood-run/README.md) and the [validation scaffold](docs/VALIDATION.md).

## Not a merge gate

`RUN_STATUS: OK` only means artifact generation completed.

Merge safety is never implied by run status.
Review state is reported separately as:

```text
DATA_COVERAGE: FULL | PARTIAL | FAILED
REVIEW_SIGNAL: BLOCKED | CLEAR | UNKNOWN
```

If data coverage is partial, the review signal is `UNKNOWN`, not `CLEAR`.
GitHub review-decision state is surfaced separately as informational reviewer state; it never overrides `REVIEW_SIGNAL` or creates blockers on its own.

## Product stance

This tool is **not a weapon to silence reviewers**.

It is a tool to enforce a stronger contract:

- reviewers may ask anything
- only evidence-backed blockers should behave like blockers
- authors should be able to answer briefly, calmly, and with traceable reasoning
- design debate should leave the PR when it stops being PR-local

## Scope for v0.1

Exactly five commands:

- `review-firewall scan`
- `review-firewall gate`
- `review-firewall draft-reply`
- `review-firewall escalate`
- `review-firewall report`

Out of scope for v0.1:

- GitHub App
- CI check integration
- automatic PR comments
- reviewer scoring
- team analytics
- policy packs
- auto-merge
- LLM-dependent core logic
- new review generation
- review history learning or scoring

## Core principles

1. **Local-first**
   The tool must run on the author’s machine.

2. **Deterministic core**
   Classification, gating, and escalation logic must work without an LLM.

3. **Stopless outputs**
   Business outcomes are represented with artifact status, not fragile shell exit semantics.

4. **Auditable artifacts**
   Every run leaves inspectable JSON/Markdown artifacts.

5. **Portable distribution**
   End users should consume release binaries, not a language runtime.

## Workspace layout

```text
crates/
  review-firewall/   # CLI binary, adapters, artifact I/O
  rf-core/           # deterministic domain logic
```

## Artifact layout

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

`latest.json` is used instead of a symlink so the run store stays Windows-compatible.

When paged GitHub PR file data is fully available, that file list is authoritative.
Local git changed files are used as a fallback for unavailable PR file data; a base-branch diff supplements the list only when changed-file metadata is explicitly partial.

For v0.1, top-level PR issue comments are kept as independent pseudo-threads unless the input carries an explicit shared thread id.
This avoids mixing unrelated PR-level conversations into one escalation signal while keeping hosted topic inference out of v0.1.

## Development

### With Nix

```bash
nix develop
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Without Nix

Use Rust stable from `rust-toolchain.toml`, plus `git`, `gh`, and `jq`.

## Release scaffolding

Release build scaffolding lives under `.github/workflows/release-build.yml` and targets:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## Release targets

Planned primary targets:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## Status

This repository is intentionally structured for small, agent-assisted changes:

- repo-level `AGENTS.md` for durable agent instructions
- project-scoped local defaults
- docs for architecture, artifact schema, and execution rules
- Rust/Nix-first contributor workflow

See [Milestones](docs/MILESTONES.md) for the reviewable PR sequence.
See [Validation](docs/VALIDATION.md) for the v0.1 dogfood metrics scaffold.
See `README_JA.md` for the Japanese guide.
