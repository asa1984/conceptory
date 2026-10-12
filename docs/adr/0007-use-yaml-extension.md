---
type: ADR
title: YAML ファイルの拡張子を .yaml に統一する
description: YAML ファイルの拡張子は .yaml に統一し、conceptory CLI で .yml のファイルを検出する。ツールが名前を決めて生成するファイルは除く。
tags:
  - yaml
  - files
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-11T01:30:02+09:00
sources:
  - id: rfc9512
    resource: https://datatracker.ietf.org/doc/html/rfc9512
    title: "RFC 9512: YAML Media Type"
---

# YAML ファイルの拡張子を .yaml に統一する

## Context and Problem Statement

YAML のファイルには `.yaml` と `.yml` の2つの拡張子が使われている。リポジトリやファイルごとに拡張子が異なると、ファイルを探すときや glob のパターンを書くときに両方を考慮する必要がある。YAML のファイルの拡張子をどう統一すべきか。

## Decision Drivers

- 拡張子が1つに決まっていること
- YAML の規格が推奨する拡張子に従うこと
- 拡張子を機械的に検査できること

## Considered Options

- a. `.yaml` に統一する
- b. `.yml` に統一する
- c. 統一しない

## Decision Outcome

採用した選択肢: **a**

YAML のメディアタイプを登録した RFC 9512 は、拡張子として `.yaml` を推奨し、`.yml` を代替として挙げている[^rfc9512]。

リポジトリで管理する YAML のファイルの拡張子は `.yaml` にし、`.yml` は使わない。ただし、ツールが名前を決めて生成するファイルは対象から外す。たとえば ast-grep の `sg test` は、`__snapshots__` ディレクトリに `<ルール名>-snapshot.yml` という名前でスナップショットを生成し、この名前は変更できない。ツールが `.yml` の名前でしか設定ファイルを読まない場合も、対象から外してよい。

この規約は conceptory CLI の機能として `conceptory files check` で検査する。`conceptory files check` は `.gitignore` で除外されていないファイルを走査し、拡張子が `.yml` のファイルを報告する。ast-grep のスナップショットは、指定しなくても検査から外す。ほかに対象から外すファイルは `--exclude` に glob のパターンで渡し、外す理由をタスクの定義のコメントに書く。実装は `crates/conceptory_files` に置く。

### Consequences

- Good: YAML のファイルを探すときや glob のパターンを書くときに、拡張子を1つだけ考えればよい。
- Good: 拡張子が規格の推奨に従う。
- Bad: 既存のリポジトリでは、`.yml` のファイルの名前を変更する必要がある。
- Bad: ツールの説明書の例が `.yml` で書かれている場合は、読み替える必要がある。

### Confirmation

CI で `conceptory files check` を実行し、`.yml` のファイルがあれば失敗させる。

## Pros and Cons of the Options

### a. `.yaml` に統一する

- Good: RFC 9512 が推奨する拡張子である。
- Bad: `.yml` を前提とするツールでは、除外の設定が必要になる。

### b. `.yml` に統一する

- Good: 拡張子が3文字で、広く使われている。
- Bad: RFC 9512 では代替の扱いである。

### c. 統一しない

- Good: 既存のファイルの名前を変更せずに済む。
- Bad: ファイルを探すときや glob のパターンを書くときに、両方の拡張子を考慮し続ける必要がある。

[^rfc9512]: RFC 9512 の application/yaml の登録では、拡張子を「"yaml" (preferred) and "yml"」としている。
