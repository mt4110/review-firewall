# review-firewall synthetic review noise cases (JA, 100)

このファイルは、`review-firewall` の **non-blocking / noisy review comments** を固定するための synthetic fixtures です。

- 件数: **100**
- 形式: YAML
- 目的: blocker にしてはいけないコメント、ADR に逃がすべきコメント、好み・運用・社会的圧力コメントの整理
- 注意: 実際の過去PRの再現ではありません。**摩擦パターン**の固定です。

## 使い方のおすすめ

- `fixtures/reviews/noise_ja.yaml` として置く
- classifier テストで `expected.blocker == false` を固定する
- `design_offtopic` は `reply == move` / `escalate == move_to_adr` を見る
- 次段で `fixtures/reviews/blockers_ja.yaml` を別途作る

## カテゴリ

- naming_style
- structure_layout
- context_not_read
- process_theater
- vague_risk
- design_offtopic
- retroactive_rules
- perf_cache
- managerial_pressure
- social_friction
