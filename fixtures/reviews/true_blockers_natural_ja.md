# review-firewall true blockers natural cases (JA, 24)

このファイルは、`review-firewall` の **自然文寄り true blocker fixtures** です。

- 件数: **24**
- 目的: きれいに構造化された blocker fixture ではなく、現実のレビューコメントに近い日本語でも blocker 境界が維持されるかを見る
- 形式: YAML
- 使い方: `rf-core` の fixture-driven tests に読み込む

## 想定カテゴリ
- correctness
- security
- performance
- operability
- api

## 注意
- これは product command ではなく test asset です
- まず `noise_exact_ja.yaml` を green にしてから取り込むのがおすすめです
