# review-firewall exact non-blocking fixtures (JA, 24)

このファイルは `review-firewall` の **non-blocking exact type** を固定するための synthetic fixtures です。

- 件数: **24**
- 内訳:
  - question: 6
  - suggestion: 6
  - nit: 6
  - praise: 6
- 目的:
  - broad noise band ではなく、**exact type** を固定する
  - concern false positive task と混ぜず、`expected.concern == null` を維持する
  - `question / suggestion / nit / praise` の日本語境界を sharp にする

## 想定用途

- `fixtures/reviews/noise_exact_ja.yaml` として repo に置く
- `crates/rf-core/tests/review_fixtures.rs` で読み込む
- すべてについて次を assert する:
  - `expected.type`
  - `expected.blocker == false`
  - `expected.concern == None`

## 設計メモ

- concern trigger 語は意図的に避けています
- runtime risk phrasing は避けています
- わざと曖昧な文にはしていません
- 「exactness を鍛える」ための fixture です
