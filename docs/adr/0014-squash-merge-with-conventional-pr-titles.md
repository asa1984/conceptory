---
type: ADR
title: PR は squash マージし、PR のタイトルを Conventional Commits の形式で cocogitto で検査する
description: main には PR ごとに1つのコミットを残し、そのコミットの件名になる PR のタイトルを CI で cocogitto で検査する。
tags:
  - git
  - ci
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-13T10:00:00+09:00
sources:
  - id: conventional-commits
    resource: https://www.conventionalcommits.org/en/v1.0.0/
    title: Conventional Commits 1.0.0
  - id: cocogitto
    resource: https://docs.cocogitto.io/
    title: Cocogitto
  - id: committed
    resource: https://github.com/crate-ci/committed
    title: crate-ci/committed
---

# PR は squash マージし、PR のタイトルを Conventional Commits の形式で cocogitto で検査する

## Context and Problem Statement

コミットメッセージを Conventional Commits の形式で書けば[^conventional-commits]、履歴を変更の種類ごとに読め、リリースのツールがバージョンや CHANGELOG を決められる。この形式を守らせるには、どのコミットを検査するかをマージの方式と合わせて決める必要がある。PR をどの方式でマージし、何をどのツールで検査すべきか。

## Decision Drivers

- main に残るコミットが必ず Conventional Commits の形式になること
- PR の途中のコミットを整える手間が少ないこと
- 検査のツールが言語に依存せず、Nix で配れること
- コミットメッセージの書き方を必要以上に縛らないこと

## Considered Options

- a. squash マージし、PR のタイトルを cocogitto で検査する
- b. squash マージし、PR のタイトルを committed で検査する
- c. マージコミットかリベースでマージし、PR のすべてのコミットを検査する

## Decision Outcome

採用した選択肢: **a**

PR は squash マージだけを許し、squash したコミットの件名には PR のタイトルを使う。main には PR ごとに1つのコミットが残るので、検査するのは PR のタイトルだけで済む。PR の途中のコミットは形式を問わない。

PR のタイトルは、CI で cocogitto の `cog verify` で検査する[^cocogitto]。cocogitto は Conventional Commits の形式と type と scope を検査し、バージョンの決定や CHANGELOG の生成も行える。committed はこれに加えて件名の長さや命令形も検査できる[^committed]。しかし件名の言い回しはレビューで直せば足り、守られなくても困ることはない。そこで機械で検査するのは、リリースのツールが読む Conventional Commits の形式だけとする。検査には、リリースのフローにも使える cocogitto を選び、ツールを1つにまとめる。

### Consequences

- Good: main の履歴は一直線になり、各コミットが Conventional Commits の形式になる。
- Good: PR の途中のコミットは、形式を気にせず書ける。
- Bad: PR の中のコミットの区切りは main に残らない。
- Bad: 大きな PR も1つのコミットにまとまるので、PR を小さく保つ必要がある。

### Confirmation

GitHub のリポジトリの設定で squash マージだけを許し、squash したコミットの件名に PR のタイトルを使う。CI の `pr-title` のジョブで PR のタイトルを検査する。

## Pros and Cons of the Options

### a. squash マージし、PR のタイトルを cocogitto で検査する

- Good: 検査とリリースの自動化を1つのツールで扱える。
- Bad: 件名の長さや命令形は検査できない。

### b. squash マージし、PR のタイトルを committed で検査する

- Good: 件名の長さや命令形まで検査できる。
- Bad: 形式が正しい PR のタイトルも、言い回しのために直す手間が増える。
- Bad: リリースの自動化には別のツールが必要になる。

### c. マージコミットかリベースでマージし、PR のすべてのコミットを検査する

- Good: PR の中のコミットの区切りが main に残る。
- Bad: PR のすべてのコミットを形式どおりに整える必要がある。

## More Information

件名の書き方を機械で揃える必要が出たときは、committed への切り替えを検討する。

[^conventional-commits]: Conventional Commits は、`<type>(<scope>): <説明>` の形式でコミットの種類と影響範囲を表す仕様である。

[^cocogitto]: `cog verify` は1つのメッセージを検査し、git のリポジトリの外でも動く。

[^committed]: committed は Conventional Commits の形式に加えて、件名の長さ、書き出しの大文字、末尾の句読点、命令形を検査できる。
