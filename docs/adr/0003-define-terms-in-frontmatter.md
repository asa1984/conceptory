---
type: ADR
title: 用語を導入した文書の frontmatter で定義し textlint で表記を検査する
description: 用語は導入した文書の frontmatter の terms で定義し、conceptory CLI が生成する prh の辞書を使って textlint で表記ゆれを検査する。
tags:
  - terminology
  - textlint
  - okf
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T01:11:01+09:00
---

# 用語を導入した文書の frontmatter で定義し textlint で表記を検査する

## Context and Problem Statement

ADR などの文書は新しい用語を導入することがある。たとえば [ADR 0001](0001-write-documents-as-okf-bundle.md) は OKF bundle と bundle-relative を導入した。同じ概念を文書ごとに別の表記で書くと、人間の読み手とコーディングエージェントはそれらを同じ概念だと判断できない。用語の表記を統一し、機械的に検査するにはどうすべきか。

## Decision Drivers

- 用語の定義が、その用語を導入した文書から離れないこと
- 日本語の表記ゆれを検査できること
- 表記の誤りを指摘するときに、用語を定義した文書を示せること

## Considered Options

- a. 用語を導入した文書の frontmatter で定義する
- b. 用語ごとの文書を用語集のディレクトリに置く
- c. prh の辞書を直接書く

どの選択肢でも、表記の検査は prh の辞書を使って textlint で行う。a と b は辞書を文書から生成する。

## Decision Outcome

採用した選択肢: **a**

用語を導入した文書がそのまま用語の定義になるので、定義と表記の規約を別の場所で管理せずに済む。

用語を導入する文書は frontmatter の `terms` に用語を列挙し、各用語の `term` に正しい表記を、`avoid` に避けるべき表記を書く。用語の定義は本文に書き、初出の箇所で用語を太字にする。

```yaml
terms:
  - term: OKF bundle
  - term: bundle-relative
    avoid:
      - バンドル相対
```

`avoid` には別の文脈で使われにくい表記だけを書く。たとえば「バンドル」は JavaScript のバンドラーの文脈でも使われるので、OKF bundle の `avoid` に書くと無関係な箇所まで指摘される。`avoid` は省略してよい。省略した用語は辞書に入れず、後述の重複と本文での定義だけを検査する。

conceptory CLI は OKF bundle のすべての文書から `terms` を集めて prh の辞書を生成する。辞書の各ルールには用語を定義した文書のパスを説明として入れるので、textlint の指摘から定義へたどれる。textlint の実行は vite-task で辞書の生成に依存させ、古い辞書で検査することを防ぐ。

conceptory CLI は用語についても次の項目を検査する。

- 同じ `term` を2つ以上の文書が定義していないこと
- ある文書の `term` が別の文書の `avoid` に含まれていないこと
- `terms` に書いた用語が、その文書の本文で太字になっていること

`deprecated-by` を持つ文書の用語は辞書に含めない。廃止された文書の用語を使い続ける場合は、廃止を決めた文書が改めて定義する。

### Consequences

- Good: 用語の定義と表記の規約が、用語を導入した文書の中にまとまる。
- Good: textlint の指摘から、用語を定義した文書へたどれる。
- Bad: prh は文字列と正規表現の一致だけで判定するので、別の文脈で使われた同じ表記も指摘することがある。
- Bad: textlint を実行する前に辞書を生成する手順が必要になる。

### Confirmation

conceptory の CI で辞書を生成してから textlint を実行する。用語の重複と衝突と本文での定義は conceptory CLI が検査する。

## Pros and Cons of the Options

### a. 用語を導入した文書の frontmatter で定義する

- Good: 用語の定義が、その用語を導入した決定と同じ文書に残る。
- Neutral: 用語の一覧を見るには conceptory CLI で集める必要がある。
- Bad: 定義が文書に散らばるので、用語の一覧を人間が通して読みにくい。

### b. 用語ごとの文書を用語集のディレクトリに置く

- Good: 用語の一覧を一か所で読める。
- Bad: 用語を導入した文書と用語集の両方を更新する必要がある。

### c. prh の辞書を直接書く

- Good: 生成の仕組みが必要ない。
- Bad: 辞書と用語の定義が別の場所にあり、辞書から定義へたどれない。

## More Information

下流リポジトリは prh の `imports` で conceptory の辞書を取り込み、自身の辞書と合わせて使う想定である。辞書の配布方法は別の ADR で定める。
