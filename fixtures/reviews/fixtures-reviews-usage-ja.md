# fixtures/reviews の使い方（JA）

## 先に結論

この fixture 群は **CLI に直接読ませるものではなく**、`rf-core` のテストで使うのが正解です。

理由:
- v0.1 のコマンド面は **5 commands 固定**
- fixture 実行のために第6コマンドを増やすと scope が濁る
- なので `cargo test` で回すのが自然

この方針は、現在の v0.1 契約（5 commands / 2 crates / deterministic core / no LLM）と一致します。

## 1. ファイルを repo に置く

```bash
mkdir -p fixtures/reviews
cp /path/to/fixtures-reviews-noise-ja-100.yaml fixtures/reviews/noise_ja.yaml
cp /path/to/fixtures-reviews-true-blockers-ja-50.yaml fixtures/reviews/true_blockers_ja.yaml
```

## 2. 使う層

使うのは `crates/rf-core` のテストです。

- `noise_ja.yaml`:
  - blocker に**してはいけない**コメント群
- `true_blockers_ja.yaml`:
  - blocker に**すべき**コメント群

## 3. 最小の導入方針

AI agent には次をやらせるのが自然です。

- `serde_yaml` を `rf-core` の **dev-dependencies** にだけ追加
- `crates/rf-core/tests/review_fixtures.rs` を追加
- YAML を読み込み、各 case を classifier / gate に流す
- `expected` と一致するかを assert する

## 4. 回すコマンド

```bash
cargo test -p rf-core review_fixtures -- --nocapture
```

または全体:

```bash
cargo test --workspace
```

## 5. 何を assert するか

### noise_ja.yaml
- `expected.blocker == false`
- `expected.type` と一致
- `expected.escalate` と一致
- `expected.reply` と一致

### true_blockers_ja.yaml
- `expected.blocker == true`
- `expected.concern` と一致
- `failure_mode_ja` / `evidence_ja` / `impact_on_pr_ja` が non-empty
- `expected.reply == accept`
- `expected.escalate == stay_in_pr`

## 6. AI agent に渡す用プロンプト

```text
Goal:
Add fixture-driven tests for synthetic review comments.

Read first:
- AGENTS.md
- Task.md
- docs/ARCHITECTURE.md
- docs/ARTIFACT_SCHEMA.md
- fixtures/reviews/noise_ja.yaml
- fixtures/reviews/true_blockers_ja.yaml

Constraints:
- Do not add a new product command.
- Keep scope fixed to the current 5 commands.
- Add YAML parsing only as a test/dev dependency if needed.
- Put fixture loading and assertions in rf-core tests, not in review-firewall CLI code.

Implement now:
1. add test-only fixture types for the YAML shape
2. load both YAML files in `crates/rf-core/tests/review_fixtures.rs`
3. assert that all noise fixtures remain non-blocking
4. assert that all true blocker fixtures remain blocking with the expected concern
5. keep tests deterministic and offline

Done when:
- `cargo test -p rf-core review_fixtures -- --nocapture` passes
- touched files and commands run are summarized
```

## 7. 重要

- これは **製品機能** ではなく **境界テスト資産** です
- つまり価値は「ユーザーに見せるUI」ではなく、「classifier を壊しにくくすること」にあります
- 先に noise 側と true blocker 側の両方を置くと、`gate` の境界がかなり安定します
