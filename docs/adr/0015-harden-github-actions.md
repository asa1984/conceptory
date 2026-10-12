---
type: ADR
title: GitHub Actions のアクションはコミットの SHA で固定し、ワークフローを actionlint と zizmor で検査する
description: CI は Nix の devShell と flake の検査を GitHub Actions で実行し、アクションの固定と最小の権限を守らせ、ワークフローを静的に検査する。
tags:
  - ci
  - github-actions
  - security
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-13T10:00:00+09:00
sources:
  - id: sha-pinning-policy
    resource: https://github.blog/changelog/2025-08-15-github-actions-policy-now-supports-blocking-and-sha-pinning-actions/
    title: GitHub Actions policy now supports blocking and SHA pinning actions
  - id: actionlint
    resource: https://github.com/rhysd/actionlint
    title: rhysd/actionlint
  - id: zizmor
    resource: https://docs.zizmor.sh/
    title: zizmor
---

# GitHub Actions のアクションはコミットの SHA で固定し、ワークフローを actionlint と zizmor で検査する

## Context and Problem Statement

CI は GitHub Actions で実行する。アクションをタグで参照すると、タグが付け替えられたときに別のコードが実行される。ワークフローには権限の与えすぎや、PR のタイトルのような外部の入力を式でそのまま埋め込む誤りも起きやすい。CI をどう構成し、ワークフローの安全をどう保つべきか。

## Decision Drivers

- 手元と CI で同じツールと同じ検査を使うこと
- 乗っ取られたアクションや、外部の入力の埋め込みの影響を受けにくいこと
- ワークフローの誤りを静的な検査で見つけられること

## Considered Options

- a. アクションを SHA で固定し、ワークフローを actionlint と zizmor で検査する
- b. アクションをタグで参照し、ワークフローはレビューで確認する

## Decision Outcome

採用した選択肢: **a**

CI では、Nix の flake の検査（`nix flake check`）と、devShell の中の `vp run check` を実行する。ツールは Nix から入れるので、手元と CI で同じバージョンを使う。

アクションはタグではなく、コミットの完全な SHA で固定し、タグをコメントで添える。Renovate の `config:best-practices` が、この形式でアクションを更新する。GitHub のリポジトリの設定でも、SHA で固定していないアクションを使うワークフローを失敗させる[^sha-pinning-policy]。ワークフローの既定の権限は `permissions: {}` とし、必要な権限をジョブごとに与える。`actions/checkout` には `persist-credentials: false` を付け、取得した認証情報を後の手順に残さない。

ワークフローは actionlint と zizmor で検査する。actionlint は構文と式の誤りを検査し、`run` のスクリプトを shellcheck で検査する[^actionlint]。zizmor は、外部の入力の埋め込み、過剰な権限、固定していないアクションのようなセキュリティの問題を検査する[^zizmor]。

### Consequences

- Good: タグが付け替えられても、固定したコミットのアクションが実行される。
- Good: ワークフローのセキュリティの問題を、マージする前に見つけられる。
- Bad: CI のたびに Nix を入れて devShell を作るので、キャッシュがなければ時間がかかる。

### Confirmation

`vp run check` の `check:actions` で actionlint と zizmor を実行する。

## Pros and Cons of the Options

### a. アクションを SHA で固定し、ワークフローを actionlint と zizmor で検査する

- Good: GitHub が推奨する固定の方法に従い、検査も自動で行える。
- Bad: SHA だけではバージョンが読めないので、タグをコメントで添える必要がある。

### b. アクションをタグで参照し、ワークフローはレビューで確認する

- Good: ワークフローが読みやすい。
- Bad: タグの付け替えによる差し替えを防げず、問題の発見をレビューに頼る。

## More Information

Nix のキャッシュと、下流のリポジトリに配る reusable workflow は、配布の方法と一緒に決める。

[^sha-pinning-policy]: GitHub は 2025 年 8 月に、許可するアクションのポリシーで SHA での固定を強制できるようにした。

[^actionlint]: actionlint は GitHub Actions のワークフローの静的な検査のツールである。

[^zizmor]: zizmor は GitHub Actions のワークフローのセキュリティを静的に検査するツールである。
