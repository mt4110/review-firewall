# Task.md

## Objective

`review-firewall` を Rust workspace として再設計し、agent-assisted な小さな改修を安全に回しながら、PR レビュー会話を作者が処理可能な形へ圧縮できる local-first CLI を完成させる。

最重要条件:

- v0.1 のスコープを広げない
- deterministic core を先に成立させる
- stopless 方針を貫く
- 監査可能な artifact を必ず残す
- Windows を含むマルチプラットフォーム配布を見据える

## Assumptions

- 実装言語は Rust stable
- ワークスペースは 2 crates 構成
- contributor 環境は Nix optional
- CLI はローカル実行前提
- GitHub 連携は `gh` CLI 経由を優先
- LLM は optional であり、v0.1 コアから切り離す

## Non-negotiable scope

実装対象は次の 5 コマンドのみ。

- `review-firewall scan`
- `review-firewall gate`
- `review-firewall draft-reply`
- `review-firewall escalate`
- `review-firewall report`

このタスクで追加しないもの:

- GitHub App
- CI integration
- 自動コメント投稿
- reviewer scoring
- team analytics
- policy packs
- 自動マージ
- LLM 依存コア

## Workspace contract

```text
crates/
  review-firewall/   # CLI binary, adapters, artifact I/O
  rf-core/           # deterministic domain logic
```

### `rf-core` responsibilities

- domain types
- comment classification
- blocker validation
- ownership advisory calculation
- escalation judgment
- draft reply selection and body assembly
- report aggregation
- artifact shape generation

### `review-firewall` responsibilities

- CLI argument parsing
- subprocess adapters (`git`, `gh`)
- config loading
- CODEOWNERS loading
- artifact writing/reading
- run directory management
- command orchestration

## Artifact contract

### Output directory

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

### Stopless rule

通常系・部分成功・エラーはすべて `status` を持つ。

最低限の status 語彙:

- `OK`
- `PARTIAL`
- `ERROR`

例:

```text
STATUS: ERROR
REASON: gh CLI returned no PR metadata
NEXT: scan_partial=true
```

例:

```json
{
  "status": "ERROR",
  "reason": "gh CLI returned no PR metadata",
  "scan_partial": true
}
```

通常の業務判断に `exit 1` を使わないこと。

## Execution phases

### Phase 0 — Freeze docs and contracts

Goal:

- `AGENTS.md`
- `docs/ARCHITECTURE.md`
- `docs/ARTIFACT_SCHEMA.md`
- `docs/adr/0001-rust-local-first-nix.md`

を先に固定する。

Acceptance:

- docs が Rust / Nix / agent-assisted workflow 方針に一致している
- symlink 廃止が明記されている
- evidence / config semantics が明文化されている

### Phase 1 — Bootstrap workspace

Goal:

- Cargo workspace 作成
- 2 crates 作成
- 共通 CI-like commands を通す最小骨格作成

Deliverables:

- root `Cargo.toml`
- `crates/rf-core/Cargo.toml`
- `crates/review-firewall/Cargo.toml`
- `src/lib.rs` / `src/main.rs`
- `flake.nix`
- `rust-toolchain.toml`

Acceptance:

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

が空実装でも通る

### Phase 2 — Domain and artifact types

Goal:

- 型を先に締める

Required types:

- `Status`
- `CommentType`
- `BlockerConcern`
- `EscalationLabel`
- `OwnershipScope`
- `AdvisoryWeight`
- `ReplyType`
- `ScanArtifact`
- `GateArtifact`
- `DraftReplyArtifact`
- `LatestPointer`

Acceptance:

- `rf-core` に I/O がない
- JSON serialize/deserialize tests がある
- artifact golden tests が通る

### Phase 3 — Implement `scan`

Goal:

- ローカル repo + GitHub 情報から `scan.json` を作る

Work:

- repo root, current branch, changed files 取得
- `review-firewall.toml` と `.github/CODEOWNERS` 存在確認
- `gh pr view --json` で PR metadata 取得
- review comments と issue comments を取得可能なら集約
- 正規化して `scan.json` を保存
- `latest.json` 更新

Acceptance:

- `gh` 失敗時も `STATUS: ERROR` or `STATUS: PARTIAL` を出す
- 何も残さず落ちない
- Windows path normalization tests がある

### Phase 4 — Implement `gate`

Goal:

- コメントを分類し、残留 blocker を抽出する

Non-negotiable logic:

- `changed path` 単独では evidence にしない
- diff comment であること単独では `present_pr_impact=true` にしない
- config flags は gate に実際に効く
- subjective preference は blocker にしない

Acceptance:

- blocker / non-blocker / downgrade tests がある
- ownership advisory tests がある
- duplicate merge tests がある

### Phase 5 — Implement `draft-reply`, `escalate`, `report`

Goal:

- 作者が短く返せる成果物を生成する

Rules:

- reply body は原則 3 行以内
- 設計論争は ADR/RFC に退避可能
- PM summary は 3 行以内

Acceptance:

- Markdown outputs are deterministic
- report tests pass
- escalation tests pass

### Phase 6 — Smoke verification

Goal:

- 実 repo で 5 コマンドを流す

Acceptance:

- `scan -> gate -> draft-reply -> escalate -> report` が動く
- GitHub 取得不可時に stopless を確認
- run artifacts が仕様通り

## What to optimize for

- auditability over cleverness
- readable refactors over premature abstraction
- typed invariants over stringly logic
- release binaries over runtime prerequisites

## What not to optimize for yet

- plugin ecosystem
- distributed workflow
- cloud-hosted inference
- reviewer reputation systems
- advanced semantic diffing
