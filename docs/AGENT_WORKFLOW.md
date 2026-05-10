# Agent Workflow

This repository is designed to work well with small, agent-assisted changes.

## What agents should use first

1. `AGENTS.md`
2. `Task.md`
3. `docs/ARCHITECTURE.md`
4. `docs/ARTIFACT_SCHEMA.md`
5. target source files

## Prompt template

Use this structure when assigning work to an AI agent:

```text
Goal:
<what must change>

Context:
<which files/docs matter>

Constraints:
<architecture, scope, safety, style>

Done when:
<tests, outputs, behaviors, docs>
```

## Recommended interaction pattern

### For large or risky work

1. start the agent in the repo root
2. ask for a plan first
3. have the agent inspect docs before code
4. implement in small steps
5. run tests and smoke checks
6. summarize touched files and remaining assumptions

### For small scoped work

Skip explicit planning only if the task is narrow and local.

## Sample prompt: implement `gate`

```text
Goal:
Implement deterministic gate logic in Rust.

Context:
Read AGENTS.md, Task.md, docs/ARCHITECTURE.md, docs/ARTIFACT_SCHEMA.md.
Implement in crates/rf-core and wire through crates/review-firewall.

Constraints:
Do not add commands. Do not rely on an LLM. changed path alone is not evidence.
Config flags must actually affect gate behavior.
Preserve stopless outputs.

Done when:
All gate tests pass, cargo fmt/clippy/test pass, and a gate.json artifact is produced in the documented shape.
```

## Suggested approval posture

Prefer tight permissions first:

- workspace write sandbox
- on-request approvals
- no network unless clearly needed

Loosen only for trusted, necessary workflows.

## Suggested project config

Keep project-local agent configuration out of product artifacts unless it is needed for contributors.

## Suggested task order for migration from the TS prototype

1. freeze docs
2. bootstrap Rust workspace
3. implement artifact/domain types
4. implement `scan`
5. implement `gate`
6. implement `draft-reply`
7. implement `escalate`
8. implement `report`
9. run smoke verification
