# review-firewall concern false positives (JA, 30)

このファイルは `extract_concern` の **false positive** を防ぐための synthetic fixtures です。

- 件数: **30**
- 形式: YAML
- 目的:
  - concern 語彙の広がりすぎを抑える
  - `question / suggestion / nit` の non-blocking band を壊さない
  - `true_blockers_ja` と対で、境界のにじみを検知する

## 方針

- すべて `expected.blocker == false`
- すべて `expected.concern == null`
- キーワードは concern 語に見えるが、文脈上は blocker concern ではない

## カテゴリ
- correctness_fp
- performance_fp
- security_fp
- operability_fp
- api_fp
