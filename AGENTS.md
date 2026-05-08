# AGENTS.md

## Mission

`review-firewall` v0.1 の目的はひとつだけです。

> PR の会話を、作者が処理できる形に圧縮すること。

このリポジトリで実装・修正・文書化を行うエージェントは、常にこの目的に従ってください。

## Product stance

- このツールは、レビュワーを黙らせる武器ではない
- このツールは、作者の認知負荷と感情負荷を下げるための local-first CLI である
- v0.1 は軽く導入できることを優先し、GitHub App や組織強制を前提にしない
- 判断は空気ではなく、構造・再現性・検証可能性に寄せる
- 曖昧な好みを blocker 面させない

## Absolute scope

v0.1 で実装対象に含めてよいコマンドは次の 5 個だけです。

- `review-firewall scan`
- `review-firewall gate`
- `review-firewall draft-reply`
- `review-firewall escalate`
- `review-firewall report`

この段階で追加してはいけないもの:

- GitHub App 連携
- CI / PR check 連携
- 自動コメント投稿
- 自動マージ
- reviewer scoring
- team analytics
- policy packs
- 複雑な AI 推論依存

## Implementation target

- 実装言語は Rust stable
- ワークスペース構成は 2 crates のみ
  - `crates/review-firewall`
  - `crates/rf-core`
- contributor 環境は Nix optional + Cargo 標準を維持する
- 配布物はマルチプラットフォームの release binaries を前提とする

## Agent workflow rules

- まず `docs/ARCHITECTURE.md` と `docs/ARTIFACT_SCHEMA.md` を読む
- 複雑な変更は実装前に plan を出す
- 仕様を変える場合は、先に docs を更新する
- 1 回の変更で scope を広げない
- 実装後は必ずテストと実行確認を行う
- AI agent への指示は Goal / Context / Constraints / Done when の 4 点で与える

## Fixed contracts

### Comment types

- `blocker`
- `question`
- `suggestion`
- `nit`
- `praise`
- `unknown`

`unknown` は内部処理用。最終表示には極力残さないこと。

### Blocker concerns

- `correctness`
- `security`
- `performance`
- `operability`
- `api`

この 5 個以外を v0.1 に追加しないこと。

### Escalation labels

- `stay_in_pr`
- `move_to_adr`
- `move_to_rfc`
- `needs_human_judgment`

### Output layout

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

**Windows 互換性のため `latest` symlink は禁止**。

## Stopless policy

このプロジェクトでは、通常の業務判断を非 0 終了コードに依存させません。

守ること:

- 通常系・部分成功・エラーは artifact の `status` に出す
- 端末出力には `STATUS:` を出す
- 失敗理由は `REASON:` で追跡可能にする
- 部分実行可能なときは後続可能性を出す

禁止:

- `gh` の失敗を、そのまま全体の破綻にすること
- 一部欠落時に artifact を何も残さず落ちること
- silent failure

例外:

- Rust の panic 相当の内部破綻
- 引数構文違反
- 読み書き不能な出力先など、プロセス継続自体が意味を失う場合

## Deterministic core first

LLM は補助機能です。骨格にしてはいけません。

LLM なしで必ず動くべきもの:

- comment type classification
- blocker schema validation
- ownership advisory
- thread roundtrip count
- residual blocker extraction
- report generation

LLM を任意で使ってよいもの:

- 長文要約
- 柔らかい返信草案の言い回し改善
- 重複コメントの自然言語圧縮
- ADR タイトル改善

## Critical semantic rules

- `changed path` だけでは evidence と見なさない
- diff 上にコメントされた事実だけでは `present_pr_impact=true` にしない
- `require_failure_mode`, `require_concern`, `require_evidence`, `require_alternative` は **設定名どおり gate に効くこと**
- ownership は advisory であり、資格の断定に使わない
- 設計論争は PR の長文応酬に閉じ込めない

## Required inputs

ローカル:

- `git diff`
- changed files
- branch name
- repo root
- `.github/CODEOWNERS`（あれば）
- `review-firewall.toml`（あれば）

GitHub:

- `gh pr view --json` で取れる PR metadata
- review comments
- issue comments（取れる場合）
- review decisions
- labels

v0.1 は `gh` を正面から使う。GraphQL 直叩きは、`gh` で表現できない穴があるときに限定する。

## Test expectations

最低限必要:

- unit tests for rf-core
- fixture / golden tests for artifact shapes
- CLI smoke tests for all 5 commands
- partial / error path tests
- Windows path normalization tests

## Done means

変更完了の条件:

- docs と実装が一致している
- `cargo fmt --all` が通る
- `cargo clippy --workspace --all-targets -- -D warnings` が通る
- `cargo test --workspace` が通る
- 変更対象コマンドの実行例を確認している
- 生成 artifact が `docs/ARTIFACT_SCHEMA.md` に従っている
