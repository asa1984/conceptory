---
type: ADR
title: GitHub Actions のワークフローの name をファイル名とそろえ、ジョブとステップには name を書かない
description: ワークフローの name は拡張子を除いたファイル名と同じにする。ジョブとステップには name を書かず、ワークフローの入力、出力、シークレットには description を書かない。
tags:
  - ci
  - github-actions
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-08T12:00:00+09:00
sources:
  - id: workflow-syntax
    resource: https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
    title: Workflow syntax for GitHub Actions
  - id: metadata-syntax
    resource: https://docs.github.com/en/actions/reference/workflows-and-actions/metadata-syntax
    title: Metadata syntax reference
---

# GitHub Actions のワークフローの name をファイル名とそろえ、ジョブとステップには name を書かない

## Context and Problem Statement

GitHub Actions のワークフロー、ジョブ、ステップには `name` で表示名を付けられ、ワークフローの入力、出力、シークレットには `description` で説明を付けられる。ところがワークフローはファイルで、ジョブは ID で既に識別されているので、表示名を自由に付けると1つのものに2つの名前ができる。そして UI の表示や status check の名前には `name` のほうが使われる。そのため、表示された名前からファイルやジョブを探すには、2つの名前の対応を覚えておく必要がある。表示名と説明をどのように書くべきか。

## Decision Drivers

- ワークフローとジョブを、ファイルと UI の両方に現れる1つの名前で指せること
- UI でワークフローを見分けやすいこと
- 名前と内容がずれないこと
- 規約を静的に検査できること

## Considered Options

- a. ワークフローの `name` をファイル名とそろえ、ジョブとステップには書かない
- b. ワークフロー、ジョブ、ステップのどれにも `name` を書かない
- c. すべてのワークフロー、ジョブ、ステップに自由に `name` を書く

## Decision Outcome

採用した選択肢: **a**

ワークフローの `name` は、拡張子を除いたファイル名と同じにする。たとえば `.github/workflows/pre-merge-check.yaml` には `name: pre-merge-check` を書く。`name` を省くと、GitHub はワークフローをリポジトリのルートからのファイルのパスで表示する[^workflow-syntax]。すべてのワークフローが `.github/workflows/` から始まるパスで並ぶので、UI では見分けにくい。ファイル名とそろえれば、短い名前で見分けられ、名前からファイルもすぐにわかる。

ジョブとステップには `name` を書かない。`name` がないジョブの status check は、ジョブの ID で表示される。ステップは `run` のコマンドや `uses` のアクションで表示されるので、ステップの内容がそのまま名前になる。

ワークフローの入力、出力、シークレットにも `description` を書かない。これらの意味は ID で表し、ID だけで伝わらない事情はコメントに書く。

対象はワークフローの構文の `name` と `description` に限る。`jobs.<job_id>.environment.name` は環境を指す識別子であり、`with` に渡す値はアクションの入力なので、どちらも対象に含めない。また、アクションのメタデータ（`action.yml`）では `name` と `description` が必須なので[^metadata-syntax]、これも対象に含めない。

### Consequences

- Good: UI に表示されたワークフローの名前から、ファイルがすぐにわかる。
- Good: ジョブの status check の名前が、そのままジョブの ID になる。
- Good: 内容を変えたときに、ジョブとステップの名前を書き換え忘れることがない。
- Bad: ワークフローのファイル名を変えるときは、`name` も変える必要がある。
- Bad: ステップの表示がコマンドそのものになるので、長いスクリプトのステップは UI で区別しにくい。
- Bad: ast-grep は指定しない限り隠しディレクトリを走査しないので、`.github` を明示して走査する必要がある。

### Confirmation

ワークフローの `name` がファイル名と同じであることは、conceptory CLI の `files check` で検査する。ast-grep のルールからはファイル名を参照できないからである。ジョブとステップの `name` は、ast-grep の `yaml/workflow-no-job-step-name` で検査する。`description` は、`yaml/workflow-no-description` で検査する。

## Pros and Cons of the Options

### a. ワークフローの `name` をファイル名とそろえ、ジョブとステップには書かない

- Good: ワークフローを UI で見分けやすく、名前は1つのままである。
- Bad: ファイル名と `name` の2か所に同じ名前を書く。

### b. ワークフロー、ジョブ、ステップのどれにも `name` を書かない

- Good: 1つのものを1つの名前で指せる。
- Bad: UI でワークフローがファイルのパスで並び、見分けにくい。

### c. すべてのワークフロー、ジョブ、ステップに自由に `name` を書く

- Good: UI に読みやすい名前を表示できる。
- Bad: ファイル名やジョブの ID と表示名の2つを保守し、対応を覚えておく必要がある。

[^workflow-syntax]: GitHub の文書によれば、`name` を省いたワークフローは、リポジトリのルートからのファイルのパスで表示される。

[^metadata-syntax]: アクションのメタデータの `name`、`description`、`runs` は必須である。
