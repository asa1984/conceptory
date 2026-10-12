---
type: ADR
title: 意思決定を MADR 4.0 形式の ADR で記録する
description: リポジトリ横断の意思決定を MADR 4.0 を基にした形式の ADR で記録する。
tags:
  - adr
  - madr
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T00:51:06+09:00
sources:
  - id: madr
    resource: https://adr.github.io/madr/
    title: MADR
---

# 意思決定を MADR 4.0 形式の ADR で記録する

## Context and Problem Statement

conceptory は asa1984 が開発し運用するアクティブなリポジトリ全体に適用する規範を制定する。規範の多くはツールの選定や配布方法のように複数の選択肢から1つを選ぶ意思決定である。決定の理由や退けた選択肢が残っていなければ、後で決定を見直すときに当時の前提が今も成り立つかを判断できない。意思決定をどのような形式で記録すべきか。

## Decision Drivers

- 退けた選択肢とその理由を残せること
- 記録がリポジトリの中に残り、コードと同じようにレビューできること
- 決定の廃止や置き換えを機械的に読み取れること

## Considered Options

- a. MADR 4.0
- b. Nygard 形式の ADR
- c. ADR を書かず issue や PR の説明に記録する

## Decision Outcome

採用した選択肢: **a**

検討した選択肢のうち、選択肢ごとの利点と欠点を書く節とメタデータを書く front matter の両方を備えるのは MADR 4.0 だけである。

ADR は `docs/adr/` に `NNNN-title-with-dashes.md` という名前で置き、番号は 0000 から振る。新しい ADR は `templates/decision.md` を複製して書く。このテンプレートは MADR 4.0 の full template を conceptory の運用に合わせて書き換えたものである。

リポジトリにある ADR は `deprecated-by` を持たない限り有効な決定として扱い、MADR の `status` は使わない。提案中の決定は PR 上で議論し、採用されたものだけを merge する。決定を廃止するときは廃止の決定そのものを新しい ADR として記録し、古い ADR の `deprecated-by` に新しい ADR を書く。古い ADR の本文は書き換えないので、廃止の理由は新しい ADR の側に残る。

### Consequences

- Good: 退けた選択肢とその理由が残るので、前提が変わったときに見直すべき選択肢がわかる。
- Good: 状態を表すのが `deprecated-by` だけになり、記録上の状態と実態がずれにくい。
- Bad: 却下した提案はリポジトリに残らず、PR の履歴をたどらなければ見つからない。
- Bad: MADR の front matter をそのまま読むツールとは互換性がない。

### Confirmation

ADR がテンプレートに沿っているかは PR のレビューで確認する。ドキュメントの検査ツールができた後は CI で検査する。

## Pros and Cons of the Options

### a. MADR 4.0

- Good: 選択肢ごとの利点と欠点を書く節がある。
- Good: メタデータを YAML front matter に書くので、機械的に読み取れる。
- Neutral: 任意の節が多く、ADR ごとに書く量を調整できる。
- Bad: テンプレートが英語なので、日本語で書くには書き換える必要がある。

### b. Nygard 形式の ADR

Michael Nygard が提案した形式で、5つの節（Title / Status / Context / Decision / Consequences）からなる。

- Good: 節が少ないので、書く手間も小さい。
- Bad: 退けた選択肢を書く節がない。
- Bad: 状態を本文の節に書くので、機械的に読み取れない。

### c. ADR を書かず issue や PR の説明に記録する

- Good: 記録のための文書を別に管理しなくてよい。
- Bad: 記録が GitHub 上にしかなく、リポジトリの中に残らない。
- Bad: 決定の後で廃止や置き換えを記録する場所がない。

## More Information

MADR の仕様とテンプレートは公式サイトで公開されている[^madr]。MADR の慣例では ADR を `docs/decisions/` に置くが、conceptory では frontmatter の `type` の値と揃う `docs/adr/` を選んだ。ADR を含むドキュメント全般の形式は [ADR 0001](0001-write-documents-as-okf-bundle.md) で定める。

[^madr]: MADR 4.0.0 は 2024-09-17 に公開された。テンプレートは full、minimal、bare の3種類である。
