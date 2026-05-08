# review-firewall

**レビューをレビューする。作者を守る。**

`review-firewall` は、PR の議論を **型付き・監査可能なレビュー信号** に圧縮する local-first の Rust CLI です。

これは:

- コードレビューを置き換える道具ではない
- CI を置き換える道具ではない
- 設計そのものを自動決定する道具ではない
- AI レビューを生成する道具ではない

やることは一つです。

> **レビューそのものをレビューすること。**

## なぜ作るのか

多くの PR では、次の 4 種類が混線します。

- 本当に壊れる話
- 設計の前提
- ローカル規約
- ただの好み

これが混ざると、PR は品質の場というより、雑音と権力の場になりやすい。
一番消耗するのは作者です。

`review-firewall` は、その混線をほどいて、曖昧な圧力を **検査可能な信号** に変えるために作ります。

## 位置づけ

`review-firewall` は、レビューコメントが既に存在した後段で動きます。

人間、AI レビューボット、CI summary、ローカルレビューCLIのどれが出したコメントでも入力にはできます。ただし、このツール自身はレビューを生成しません。役割は、作者にとって本当に対応すべきものを整理することです。どれが根拠付き blocker で、どれが question / suggestion に落ち、どの設計論争を PR 外へ出すべきかを artifact にします。

境界の詳細は [Product Boundary](docs/PRODUCT_BOUNDARY.md) を見てください。

## プロダクトの立場

これは **レビュワーを黙らせる武器** ではありません。

正しい立場はこうです。

- レビュワーは自由に質問してよい
- ただし blocker として振る舞うなら、根拠と影響が必要
- 作者は短く、冷静に、証拠付きで返せるべき
- 設計論争は PR の外に退避できるべき

## v0.1 のスコープ

コマンドは 5 つだけです。

- `review-firewall scan`
- `review-firewall gate`
- `review-firewall draft-reply`
- `review-firewall escalate`
- `review-firewall report`

v0.1 でやらないもの:

- GitHub App
- CI 連携
- PR への自動コメント
- reviewer scoring
- team analytics
- policy packs
- 自動マージ
- LLM 依存のコア判定
- AI レビュー生成
- review history による学習や scoring

## 基本原則

1. **local-first**
   作者の手元で動くことを優先する。

2. **deterministic core**
   分類・gating・escalation は LLM なしで成立させる。

3. **stopless**
   業務判断は exit code ではなく artifact の `status` で表現する。

4. **監査可能性**
   すべての実行で JSON / Markdown artifact を残す。

5. **配布容易性**
   利用者は言語ランタイムではなく、配布バイナリを使えるべき。

## ワークスペース構成

```text
crates/
  review-firewall/   # CLI binary, adapters, artifact I/O
  rf-core/           # deterministic domain logic
```

## Artifact 構成

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

Windows 互換性のため、`latest` symlink ではなく `latest.json` を使います。

v0.1 では、PR 全体の issue comment を `escalate` が長い設計論争として見落とさないよう、粗い pseudo-thread に畳み込むことがあります。

## 開発

### Nix を使う場合

```bash
nix develop
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Nix を使わない場合

`rust-toolchain.toml` の stable ツールチェーンと、`git`, `gh`, `jq` を用意してください。

## リリースビルドの足場

リリース用の build matrix は `.github/workflows/release-build.yml` に置きます。
対象は次の 5 ターゲットです。

## 想定配布ターゲット

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

## この repo の使い方

このリポジトリは、小さく安全な agent-assisted 変更を回しやすいように作ります。

- `AGENTS.md` で永続的な制約を定義
- project-local な設定で作業環境を固定
- `docs/` で設計と artifact 契約を固定
- Rust + Nix を前提に contributor 体験を揃える

レビューしやすい PR の区切りは [Milestones](docs/MILESTONES.md) に置きます。
