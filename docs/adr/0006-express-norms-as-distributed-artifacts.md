---
type: ADR
title: 横断の規範は配布物で示し ADR には決定の理由を記録する
description: 下流のリポジトリが従う規範はスタイルガイドや skill などの配布物で示し、ADR は conceptory の意思決定とその理由の記録として docs/adr にまとめる。
tags:
  - adr
  - distribution
  - okf
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T04:03:12+09:00
---

# 横断の規範は配布物で示し ADR には決定の理由を記録する

## Context and Problem Statement

conceptory の ADR には2種類ある。下流のリポジトリが従う規範を決めたもの（MADR で記録する、OKF で書くなど）と、conceptory 自身の実装を決めたもの（conceptory CLI を Rust で実装するなど）である。[ADR 0003](0003-define-terms-in-frontmatter.md) や [ADR 0005](0005-check-japanese-in-two-stages.md) のように、1つの ADR が規範とその実現方法の両方を含むこともある。下流のリポジトリは規範に従う必要があるが、ADR には決定の経緯や conceptory に固有の事情も書かれている。ADR をどう扱い、規範を下流にどう示すべきか。

## Decision Drivers

- 下流のリポジトリが conceptory の ADR を読まなくても規範に従えること
- 1つの決定を不自然に分割しないこと
- 規範とその理由の対応をたどれること

## Considered Options

- a. ADR は理由の記録として docs/adr にまとめ、規範は配布物で示す
- b. ADR を conceptory 自身のものと横断のものに分けて置く
- c. 横断の規範を決めた ADR を下流へ配布する

## Decision Outcome

採用した選択肢: **a**

ADR を規範と実現方法に分ける必要がなく、下流のリポジトリは配布物だけで規範に従える。

conceptory の ADR は、規範に関わるかどうかを問わず `docs/adr/` に置く。`docs/` は conceptory 自身の記録の OKF bundle である。下流のリポジトリが従う規範そのものは、配布物として示す。配布物は、`guides/` に置くスタイルガイド、`skills/` の skill、`schemas/` のスキーマ、conceptory CLI などのツール、各種の設定ファイルである。`guides/` は `docs/` とは別の OKF bundle とし、conceptory CLI は2つの OKF bundle をまとめて検査する。

規範を変更するときは、まず ADR で決定し、その決定に合わせて配布物を更新する。ある規範が横断のものかどうかは、ADR の分類ではなく、その規範を示す配布物があるかどうかで決まる。

### Consequences

- Good: 下流のリポジトリは配布物だけを参照すれば規範に従える。
- Good: 規範と実現方法を含む決定を、1つの ADR のまま記録できる。
- Bad: 規範を変更するときは、ADR と配布物の両方を更新する必要がある。
- Bad: 下流のリポジトリが規範の理由を知るには、conceptory の ADR を参照する必要がある。

### Confirmation

規範を変更する PR では、対応する配布物が更新されているかをレビューで確認する。

## Pros and Cons of the Options

### a. ADR は理由の記録として docs/adr にまとめ、規範は配布物で示す

- Good: ADR を分割せずに済む。
- Good: 配布物には conceptory の外でも通用する内容だけを書けばよい。
- Bad: 規範と ADR の対応は検査できないので、人間が管理し続ける。

### b. ADR を conceptory 自身のものと横断のものに分けて置く

- Good: 横断の決定を一覧しやすい。
- Bad: 規範と実現方法の両方を含む決定を、2つの ADR に分割する必要がある。

### c. 横断の規範を決めた ADR を下流へ配布する

- Good: 下流のリポジトリが規範の理由まで読める。
- Bad: ADR には conceptory に固有の事情や経緯が含まれるので、下流には不要な情報まで届く。
- Bad: ADR は廃止しても本文を書き換えないので、配布した ADR から現在の規範を読み取りにくい。
