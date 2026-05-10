# review-firewall true blocker cases (JA, 50)

このファイルは `review-firewall` の **本当に blocker 扱いすべきレビューコメント** を固定するための synthetic fixtures です。

- 件数: **50**
- 形式: YAML
- 目的: `noise_ja` と対照で、`gate` の境界をテストする
- 注意: 実在PRの再現ではなく、**境界学習用の synthetic fixtures** です

## 想定カテゴリ

- correctness
- security
- performance
- operability
- api

## 使い方の基本

- `fixtures/reviews/true_blockers_ja.yaml` に置く
- `fixtures/reviews/noise_ja.yaml` と同時に読み込む
- classifier / gate の fixture tests で `expected.blocker == true` を固定する
- `failure_mode_ja`, `evidence_ja`, `impact_on_pr_ja` が揃っているかを見る
