# ADR 0001: Rust, local-first, Nix-supported contributor workflow

- Status: Accepted
- Date: 2026-03-28

## Context

`review-firewall` started as a TypeScript prototype to freeze behavior and command boundaries.
That prototype validated the top-level command flow, but it is not the right long-term foundation.

The long-term requirements are:

- local-first execution
- deterministic core logic
- auditable artifacts
- multi-platform binary distribution
- low runtime assumptions for end users
- safe, repeated agent-assisted modification

## Decision

We will rebuild v0.1 as a Rust workspace with exactly two crates:

- `crates/rf-core`
- `crates/review-firewall`

We will support Nix for reproducible contributor environments, but **Nix is not required to run the released product**.
End users consume release binaries.

We will keep the TypeScript prototype only as a behavior reference during migration.
It is not the production foundation.

## Rationale

### Why Rust

Rust matches the long-term shape of this project:

- domain types can be explicit and closed
- classification and gating logic benefit from enums and exhaustive matching
- release binaries are easy to ship
- cross-platform support is straightforward
- refactors are less likely to silently break semantics

### Why not keep TypeScript as the mainline

The prototype uses almost none of the ecosystem benefits that would justify shipping a Node runtime requirement.
For this product, runtime independence matters more than script-level convenience.

### Why Nix

Nix gives contributors and agent-assisted development a reproducible environment.
That reduces setup drift between machines and CI-like local checks.

### Why only two crates

This repository will evolve through many small rule changes.
Too many crates would increase build overhead and make iteration harder.
Two crates are enough for v0.1:

- one pure domain core
- one I/O and orchestration shell

## Consequences

### Positive

- stronger invariants
- better artifact compatibility discipline
- easier multi-platform binary releases
- less runtime friction for users

### Negative

- slower build cycles than the TS prototype
- more up-front type design
- migration work before new features

## Non-goals

This ADR does not introduce:

- GitHub App integration
- CI as required runtime
- any LLM dependence in the core
- a reviewer reputation system

## Follow-up decisions

- `latest` symlink is replaced with `latest.json`
- `changed path` alone is not valid evidence
- config semantics must match actual gate behavior
- issue comments may be incorporated when available, but stopless behavior remains mandatory
