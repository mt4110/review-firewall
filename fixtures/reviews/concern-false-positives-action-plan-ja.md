# concern_false_positives_ja 導入アクションプラン（超詳細）

## 先に結論

この fixture は **製品機能ではなく境界テスト資産** です。
やることは 3 つだけです。

1. fixture を repo の `fixtures/reviews/` に置く
2. `rf-core` のテストで YAML を読む
3. `extract_concern` が `null` のままであることを固定する

ここで **新しい product command は増やしません**。
`scan` / `gate` / `draft-reply` / `escalate` / `report` の 5 コマンドはそのままです。
この方針は v0.1 の scope と一致します。

---

## この fixture の役割

既存の fixture は大きく 2 系統あります。

- `noise_ja`
  - blocker ではないコメント群
- `true_blockers_ja`
  - blocker であるべきコメント群

今回の `concern_false_positives_ja` は、その中間というより **副作用抑制用** です。

役割はこれです。

- concern 語彙を増やした結果、
  - `sort`
  - `lock`
  - `header`
  - `origin`
  - `owner`
  - `schema`
  のような単語だけで concern が誤抽出されるのを防ぐ
- つまり **“単語がある” と “concern がある” を分ける**

これは review-firewall の本質にかなり合っています。
この OSS は「賢そうに見える」よりも、**うかつに blocker っぽくしない**方が重要だからです。

---

## 置き場所

repo ではこの場所に置くのがおすすめです。

```bash
cp /path/to/fixtures-reviews-concern-false-positives-ja-30.yaml   review-firewall/fixtures/reviews/concern_false_positives_ja.yaml
```

補助の説明ファイルも置くなら:

```bash
cp /path/to/fixtures-reviews-concern-false-positives-ja-30.md   review-firewall/fixtures/reviews/concern_false_positives_ja.md
```

---

## 実装の基本方針

### 触る場所
- `crates/rf-core/tests/review_fixtures.rs`
- 必要なら `crates/rf-core/Cargo.toml` の dev-dependencies
- 必要なら test-only fixture struct

### 触らない場所
- `crates/review-firewall/src/command/*`
- adapter 層
- artifact schema
- runtime YAML parsing
- product commands

---

## テストでやること

### 最低限の assertion
各 case について:

- `expected.blocker == false`
- `expected.concern == null`
- 実際の concern 抽出結果も `None` / `null`
- 可能なら `type` も broad non-blocking band に入っていることを確認

### 優先順位
1. **concern が誤って付かない**
2. blocker にならない
3. type の exact 一致は今回は必須ではない

ここはかなり大事です。
この fixture の主目的は **exact type** ではなく **false-positive concern suppression** です。

---

## 推奨テスト名

- `noise_fixtures_remain_non_blocking`
- `true_blocker_fixtures_remain_blocking`
- `concern_false_positive_fixtures_do_not_extract_concern`

3 本に分けると見やすいです。

---

## AI agent にやらせる具体作業

### Phase A: fixture 追加
- `fixtures/reviews/concern_false_positives_ja.yaml` を repo に置く

### Phase B: fixture loader 拡張
- 既存の test-only YAML struct がそのまま使えるか確認
- 使えなければ、unknown field tolerant な struct を最小追加

### Phase C: assertion 追加
- concern_false_positive fixture を読み込む
- 各 case について:
  - classifier / gate input を最小構築
  - concern が `None`
  - blocker が false
  を確認

### Phase D: verify
```bash
cargo fmt --all
cargo fmt --all --check
cargo clippy -p rf-core --all-targets --all-features --offline -- -D warnings
cargo test -p rf-core review_fixtures -- --nocapture
cargo test --workspace --offline
```

---

## 失敗しやすい点

### 1. 既存の `noise_ja` と役割が混ざる
この fixture は `noise_ja` の重複ではありません。
主眼は **concern 誤抽出** です。

### 2. broad keyword を増やしすぎた副作用を見落とす
たとえば:
- `sort`
- `header`
- `owner`
- `schema`
- `origin`

このへんは単語だけだと危ないです。
今回の fixture は、まさにそこを刺します。

### 3. exact type まで一気に厳密化しようとして task を太らせる
今回はそこまでやらない方がいいです。
まずは concern suppression に集中する。

---

## Done 条件

以下を満たしたら完了でいいです。

- fixture ファイルが `fixtures/reviews/concern_false_positives_ja.yaml` として置かれている
- `crates/rf-core/tests/review_fixtures.rs` で読み込まれる
- すべての case で concern が `null` のまま
- すべての case で blocker にならない
- `cargo test -p rf-core review_fixtures -- --nocapture` が pass
- `cargo test --workspace --offline` が pass
- product scope は増えていない

---

## 次にやると良い順番

この fixture を入れたあと、自然な順番はこうです。

1. `concern_false_positives_ja.yaml`
2. `noise_exact_ja.yaml`
3. `true_blockers_natural_ja.yaml`
4. config matrix fixtures

この順がいい理由:
- まず過剰反応を止める
- そのあと分類を細くする
- そのあと自然文 blocker に広げる
- 最後に config semantics を matrix で締める

---

## AI agent にそのまま渡す長めプロンプト

```text
Goal:
Add concern false-positive fixtures to the existing rf-core fixture-driven tests.

Read first:
- AGENTS.md
- Task.md
- docs/ARCHITECTURE.md
- docs/ARTIFACT_SCHEMA.md
- docs/REVIEW_CONSTITUTION.md
- fixtures/reviews/fixtures-reviews-noise-ja-100.yaml
- fixtures/reviews/fixtures-reviews-true-blockers-ja-50.yaml
- fixtures/reviews/concern_false_positives_ja.yaml

Context:
This repo already has fixture-driven tests for synthetic review comments.
The current task is to harden concern extraction against false positives.
Do not widen scope into new commands or new product behavior.

Hard constraints:
- Do not add a new product command.
- Keep scope fixed to the current 5 commands.
- Keep exactly 2 crates.
- Keep YAML parsing in test/dev paths only.
- Put all fixture loading and assertions in rf-core tests.
- Do not add runtime YAML parsing.
- Do not change CLI / adapters / artifact schemas for this task.
- Keep tests deterministic and offline.

Primary objective:
Ensure that synthetic comments containing broad concern-like tokens do NOT get a concern assigned unless the surrounding text actually justifies it.

Implement now:
1. load `fixtures/reviews/concern_false_positives_ja.yaml`
2. reuse or minimally extend the test-only fixture structs
3. add a dedicated test:
   - `concern_false_positive_fixtures_do_not_extract_concern`
4. assert for every case:
   - expected.blocker == false
   - actual result is non-blocking
   - actual extracted concern is None/null
5. keep the patch small and contained to rf-core tests and tiny pure helpers only if unavoidable

Important:
- This task is about suppressing false positives in concern extraction.
- Do not overfit by adding many ad hoc special cases.
- Prefer the smallest deterministic improvement if a helper refactor is needed.
- If a fixture seems wrong, stop and report the fixture IDs instead of silently rewriting them.

Suggested touched files:
- crates/rf-core/tests/review_fixtures.rs
- maybe crates/rf-core/src/classify.rs if a tiny pure helper adjustment is absolutely needed
- maybe crates/rf-core/Cargo.toml only if test deps truly need adjustment

Verification commands:
- cargo fmt --all
- cargo fmt --all --check
- cargo clippy -p rf-core --all-targets --all-features --offline -- -D warnings
- cargo test -p rf-core review_fixtures -- --nocapture
- cargo test --workspace --offline

Done when:
- all concern false-positive fixtures pass
- no new product command exists
- no runtime YAML parsing was added
- touched files, commands run, and remaining assumption gaps are summarized

End-of-task report format:
## Goal
## Plan used
## Touched files
## Commands run
## Test results
## Remaining assumption gaps
## Suspicious fixture IDs (if any)
## Why this patch stays within v0.1 scope
```

---

## 人間がレビューするときの見るポイント

### 良い匂い
- `review_fixtures.rs` だけでほぼ完結している
- `classify.rs` を触っても最小限
- concern 語彙の抑制が pure helper で行われる
- CLI 側は無傷

### 悪い匂い
- 新しい product command を生やす
- fixture 実行専用の binary を作る
- YAML parsing を runtime 側に入れる
- false positive 抑制のために ad hoc な if 文が大量に増える

---

## もし今日これ以上やらないなら

やる順番はこれで十分です。

1. fixture ファイルを置く
2. AI agent に上の prompt を投げる
3. 朝起きたら:
   - touched files
   - commands run
   - test results
   - assumption gaps
   だけ見る

これでいいです。
