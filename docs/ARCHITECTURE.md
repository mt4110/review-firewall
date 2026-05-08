# Architecture

## North star

`review-firewall` converts PR discussion into typed, auditable review signals and author-ready evidence.

The product is intentionally **local-first**, **deterministic**, and **artifact-driven**.

It is deliberately downstream of review generation.
Human reviewers, AI review bots, CI summaries, or local review tools may produce comments; `review-firewall` classifies and routes those comments into author-ready artifacts.

It must not become a diff-to-review generator.

## High-level shape

```text
Local repo + gh/git + existing PR comments
        |
        v
crates/review-firewall (CLI shell)
  - command parsing
  - subprocess adapters
  - config / CODEOWNERS I/O
  - artifact persistence
        |
        v
crates/rf-core (pure domain core)
  - normalization
  - classification
  - blocker validation
  - ownership advisory
  - escalation judgment
  - draft reply generation
  - report aggregation
        |
        v
.review-firewall/run/<timestamp>/...
```

## Workspace layout

```text
Cargo.toml
flake.nix
rust-toolchain.toml
AGENTS.md
Task.md
README.md
README_JA.md
docs/
  ARCHITECTURE.md
  ARTIFACT_SCHEMA.md
  AGENT_WORKFLOW.md
  adr/
    0001-rust-local-first-nix.md
crates/
  review-firewall/
    Cargo.toml
    src/
      main.rs
      cli.rs
      command/
        mod.rs
        scan.rs
        gate.rs
        draft_reply.rs
        escalate.rs
        report.rs
      adapter/
        mod.rs
        git.rs
        gh.rs
      io/
        mod.rs
        config.rs
        codeowners.rs
        run_store.rs
        artifacts.rs
  rf-core/
    Cargo.toml
    src/
      lib.rs
      domain/
        mod.rs
        status.rs
        comment.rs
        blocker.rs
        ownership.rs
        escalation.rs
        reply.rs
        artifact.rs
      normalize.rs
      classify.rs
      ownership.rs
      escalation.rs
      draft_reply.rs
      report.rs
      dedupe.rs
```

## Crate boundaries

### `rf-core`

Pure logic only.

Allowed:

- serde-friendly domain types
- pure classification rules
- escalation rules
- report composition
- string normalization helpers

Forbidden:

- filesystem I/O
- subprocess execution
- direct environment reads
- network access

### `review-firewall`

Shell around the core.

Allowed:

- CLI parsing
- stdout/stderr printing
- reading/writing files
- subprocesses (`git`, `gh`)
- timestamp generation
- env/config resolution

Forbidden:

- re-implementing classification logic outside `rf-core`
- embedding business rules in ad-hoc CLI branches

## Domain model

### Status

- `OK`
- `PARTIAL`
- `ERROR`

### Comment type

- `Blocker`
- `Question`
- `Suggestion`
- `Nit`
- `Praise`
- `Unknown`

### Blocker concern

- `Correctness`
- `Security`
- `Performance`
- `Operability`
- `Api`

### Escalation label

- `StayInPr`
- `MoveToAdr`
- `MoveToRfc`
- `NeedsHumanJudgment`

### Ownership scope

- `Exact`
- `Partial`
- `None`

### Advisory weight

- `High`
- `Medium`
- `Low`

### Reply type

- `Accept`
- `Decline`
- `Move`

## Command contracts

### `scan`

Collect and normalize:

- repo root
- branch
- changed files
- CODEOWNERS presence from `.github/CODEOWNERS`, `CODEOWNERS`, or `docs/CODEOWNERS`
- config presence
- PR metadata via `gh`
- review comments
- issue comments when available

For v0.1, general PR issue comments are kept as independent pseudo-threads unless the input already carries an explicit shared thread id.
This avoids inferring one long debate from unrelated top-level comments without adding hosted topic inference.

Output:

- `scan.json`
- stdout summary

### `gate`

Compute:

- normalized comments
- comment type
- candidate blockers
- residual blockers
- ownership advisory
- aggregate counts

The PR author's own comments may still be classified for context, but they are not extracted as residual blockers.
Diff-local context by itself is never enough for `present_pr_impact=true`; the comment must include a concrete failure mode.

Output:

- `gate.json`
- stdout summary

### `draft-reply`

Generate concise author replies.
If local config is partial, the command status and reason reflect that instead of silently using defaults.

Output:

- `draft_reply.json`
- `draft_reply.md`

### `escalate`

Turn long design threads into ADR/RFC draft material.
If local config is partial, the command status and reason reflect that because the roundtrip threshold may have fallen back to defaults.

Output:

- `escalation.md`

### `report`

Produce final engineer + PM + author outputs.
The report command merges status from scan, gate, draft-reply, and escalation artifacts so final output cannot hide a partial downstream command.

Output:

- `report.md`

## Semantics that must not drift

### Evidence

The following are **not enough by themselves**:

- comment exists on a changed line
- changed file path mentions a risky subsystem
- reviewer sounds confident

Evidence should come from at least one of:

- concrete failure mode
- repro condition
- contract break explanation
- benchmark or measured impact
- operational failure path
- explicit reasoning tied to the changed behavior

### Present PR impact

A comment does not become PR-impacting just because it is attached to the diff.
The impact must be tied to changed behavior or contract surface.

### Config semantics

If config says `require_evidence = false`, the gate logic must really relax evidence.
Config flags must not be decorative.

## Run store design

```text
.review-firewall/
  run/
    latest.json
    20260328T203500Z/
      scan.json
      gate.json
      draft_reply.json
      draft_reply.md
      escalation.md
      report.md
```

Why `latest.json` instead of symlink:

- Windows friendliness
- simpler tooling
- easier deterministic tests

Run directory timestamps include nanosecond precision so repeated `scan` executions do not reuse the same directory.
If a timestamp collision still occurs on a coarse clock, the run directory gets a numeric suffix such as `-01` instead of sleeping.
When `latest.json` is read back, the timestamp must match a generated run-directory name before it is joined to the run root.

## Testing strategy

### Unit tests

`rf-core`

- classification
- blocker validation
- ownership advisory
- escalation rules
- report generation

### Fixture / golden tests

- artifact shapes
- markdown outputs
- downgrade cases
- config semantics

### CLI smoke tests

- each command
- stopless error path
- empty / partial inputs

### Platform tests

- path normalization
- run store behavior on Windows-style paths

## Release design

Artifacts are distributed as release binaries.
Contributor environment can use Nix, but product runtime must not require it.
