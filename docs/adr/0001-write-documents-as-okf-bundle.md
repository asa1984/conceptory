---
type: ADR
title: ドキュメントを OKF bundle として書く
description: conceptory と下流リポジトリのドキュメントを Open Knowledge Format v0.2 の OKF bundle として書き、検査ツールで検査する。
tags:
  - okf
  - documentation
terms:
  - term: OKF bundle
  - term: bundle-relative
    avoid:
      - bundle relative
      - バンドル相対
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T00:51:06+09:00
sources:
  - id: okf-spec
    resource: https://github.com/GoogleCloudPlatform/open-knowledge-format/blob/main/SPEC.md
    title: Open Knowledge Format Specification v0.2
---

# ドキュメントを OKF bundle として書く

## Context and Problem Statement

conceptory は ADR や原則などのドキュメントを制定し、下流のリポジトリもそれぞれドキュメントを持つ。これらのドキュメントは人間だけでなくコーディングエージェントも読むので、文書の種類や文書どうしの関係を機械的に読み取れる必要がある。しかし書き方が文書ごとに異なれば、エージェントは文書を本文から推測して読むしかなく、形式を検査するツールも作れない。ドキュメントをどのような形式で書くべきか。

## Decision Drivers

- コーディングエージェントが文書の種類と文書どうしの関係をたどれること
- 特定のベンダーやツールに依存しない形式であること
- 形式を検査ツールで検査できること

## Considered Options

- a. Open Knowledge Format（OKF）v0.2
- b. conceptory 独自の frontmatter 規約
- c. frontmatter を持たない Markdown

## Decision Outcome

採用した選択肢: **a**

OKF はベンダーに依存しない公開仕様であり、文書の種類を表す `type` と文書どうしのリンクの規則だけを定める[^okf-spec]。conceptory が必要とするフィールドはこの上に追加できるので、規約を一から設計する必要がない。

OKF は、YAML frontmatter 付きの Markdown ファイルを収めたディレクトリツリーを **OKF bundle** と呼ぶ。その最上位のディレクトリが OKF bundle のルートである。リポジトリのドキュメントは `docs/` を OKF bundle として書き、ルートの `index.md` で `okf_version` を宣言する。conceptory は、下流へ配布する規範を別の OKF bundle の `guides/` に置く（[ADR 0006](0006-express-norms-as-distributed-artifacts.md)）。各ディレクトリには配下の文書を一覧にする `index.md` を置く。`type` の値は文書の種類ごとに ADR で定め、ADR 自身の `type` は `ADR` とする。

OKF は `/` で始まるパスを OKF bundle のルートを起点に解釈し、この書き方を **bundle-relative** と呼ぶ。frontmatter のうちパスを値に取るフィールドには bundle-relative のパスを書く。どの文書から参照しても同じ値になるので、機械的に扱いやすいからである。一方で本文中のリンクには相対パスを書く。GitHub は `/` で始まるリンクをリポジトリのルートを起点に解決するので、本文に bundle-relative のパスを書くと GitHub 上でリンクが切れる。

OKF の `status` は使わない。文書は `deprecated-by` を持たない限り有効とみなし、廃止された文書の `deprecated-by` には廃止を決めた文書を bundle-relative のパスで書く。OKF は `status` を省略した文書を `stable` とみなすので、有効な文書については OKF の解釈と一致する。

検査ツールは conceptory で実装し、OKF への適合と conceptory が定めた frontmatter のスキーマと `deprecated-by` の参照先を検査する。下流リポジトリへの配布方法は別の ADR で定める。

### Consequences

- Good: 文書の種類と関係が frontmatter とリンクに現れるので、エージェントは必要な文書を選んで読める。
- Good: 仕様が小さいので、検査ツールを自前で実装しやすい。
- Bad: OKF は 2026-06 に公開されたばかりであり、今後の改訂に破壊的な変更を含む可能性がある。
- Bad: `deprecated-by` は conceptory 独自のフィールドなので、ほかの OKF consumer は廃止された文書も `stable` として扱う。
- Bad: frontmatter と本文でパスの書き方が異なるので、書き手は2つの規則を使い分ける必要がある。

### Confirmation

検査ツールを CI で実行し、`docs/` と `guides/` が OKF に適合して conceptory の frontmatter のスキーマを満たしていることを確認する。検査ツールができるまでは PR のレビューで確認する。

## Pros and Cons of the Options

### a. Open Knowledge Format（OKF）v0.2

- Good: ベンダーに依存しない公開仕様である。
- Good: 必須のフィールドが `type` だけなので、既存の Markdown にも後から適用できる。
- Bad: 仕様がまだ若く、v0.1 から v0.2 への改訂でもフィールドの名前が変わった。

### b. conceptory 独自の frontmatter 規約

- Good: conceptory に必要なフィールドだけを自由に設計できる。
- Bad: 規約の設計と文書化を一から行う必要がある。
- Bad: conceptory の外のツールやエージェントは規約を知らない。

### c. frontmatter を持たない Markdown

- Good: 書くための規則がなく、負担が最も小さい。
- Bad: 文書の種類や廃止の有無を機械的に読み取れないので、検査ツールを作れない。

## More Information

OKF の仕様と参照実装は GitHub の GoogleCloudPlatform/open-knowledge-format リポジトリで公開されている。参照実装は OKF bundle を生成するエージェントと可視化ツールであり、OKF bundle を検査するツールは含まれていない。

[^okf-spec]: Open Knowledge Format Specification v0.2。必須の frontmatter は `type` だけであり、`status` を省略した文書は `stable` とみなされる。
