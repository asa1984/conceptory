---
type: ADR
title: マージを止めるジョブを pre-merge-check ジョブの needs に集め、それだけを必須の status check にする
description: PR のマージを止めるジョブは pre-merge-check.yaml の pre-merge-check ジョブの needs に並べ、ruleset の必須の status check には pre-merge-check だけを登録する。
tags:
  - ci
  - github-actions
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-04T12:00:00+09:00
sources:
  - id: control-jobs-with-conditions
    resource: https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-jobs-with-conditions
    title: Using conditions to control job execution
  - id: workflow-syntax
    resource: https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
    title: Workflow syntax for GitHub Actions
  - id: troubleshooting-required-status-checks
    resource: https://docs.github.com/en/pull-requests/how-tos/merge-and-close-pull-requests/troubleshooting-required-status-checks
    title: Troubleshooting required status checks
---

# マージを止めるジョブを pre-merge-check ジョブの needs に集め、それだけを必須の status check にする

## Context and Problem Statement

PR のマージを CI の結果で止めるには、ruleset の必須の status check にジョブを登録する。しかしジョブを1つずつ登録すると、ジョブを加えたり名前を変えたりするたびにリポジトリの設定も変える必要がある。この設定はリポジトリのファイルの外にあるのでレビューされず、登録を忘れたジョブは失敗してもマージを止めない。マージの条件をどのように定めるべきか。

## Decision Drivers

- ジョブを加えたときに、リポジトリの設定を変えずにマージの条件に含められること
- マージの条件をワークフローのファイルで定め、レビューできること
- 規約を静的に検査できること

## Considered Options

- a. `pre-merge-check` ジョブの `needs` に集め、それだけを必須の status check にする
- b. ジョブを1つずつ必須の status check に登録する

## Decision Outcome

採用した選択肢: **a**

PR のマージを止めるジョブは `.github/workflows/pre-merge-check.yaml` に置き、同じファイルの `pre-merge-check` ジョブの `needs` に並べる。ruleset の必須の status check には `pre-merge-check` だけを登録する。並列で実行したいジョブは、別のジョブとして `needs` に加えればよい。ジョブには ADR 0016「GitHub Actions のワークフローの name をファイル名とそろえ、ジョブとステップには name を書かない」に従って `name` を書かない。そのため、status check の名前はジョブの ID と同じ `pre-merge-check` になる。

ただし、`needs` のジョブが失敗すると、条件を書かない限り後のジョブはスキップされる[^workflow-syntax]。そして、スキップされたジョブは必須の status check であっても成功として扱われる[^control-jobs-with-conditions]。そのため、`pre-merge-check` には `if: always()` を書いて常に実行し、`needs` のジョブが失敗したかキャンセルされたときはステップを失敗させる。条件によってスキップされた `needs` のジョブは、失敗として扱わない。

```yaml
pre-merge-check:
  if: always()
  needs: [build, test]
  runs-on: ubuntu-slim
  timeout-minutes: 5
  steps:
    - if: contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled')
      run: exit 1
```

`needs` に書けるのは同じワークフローのジョブだけなので、マージを止めるジョブはすべて `pre-merge-check.yaml` に置く。ワークフローとジョブの名前をそろえておけば、マージの条件がどのファイルにあるかを探さずに済む。また、`needs` のジョブがさらに別のジョブを `needs` に持つときは、そのジョブも `pre-merge-check` の `needs` に書く。前のジョブが失敗すると後のジョブはスキップされるので、前のジョブの結果を `pre-merge-check` が見なければ、検査しないままマージできてしまう。

`pre-merge-check.yaml` には `paths` のようなフィルターを付けない。ワークフローごとスキップされると、その status check は保留のままになり、マージを止め続けるからである[^troubleshooting-required-status-checks]。変更したファイルによって検査を省きたいときは、ワークフローではなくジョブをスキップする。変更したファイルを判定するジョブを置いて結果を `outputs` で渡し、後のジョブの `if` で参照すればよい。判定には `dorny/paths-filter` のようなアクションを使える。条件によってスキップされたジョブは失敗として扱わないので、マージを止めない。

```yaml
rust:
  needs: [changes]
  if: needs.changes.outputs.rust == 'true'
pre-merge-check:
  if: always()
  needs: [changes, rust]
```

### Consequences

- Good: ジョブを加えても、`needs` に書けばリポジトリの設定を変えずにマージの条件に含められる。
- Good: マージの条件がワークフローのファイルにあるので、変更をレビューできる。
- Bad: マージを止めるジョブを、ワークフローをまたいで分けられない。
- Bad: `pre-merge-check` のために runner を1つ余計に起動する。
- Bad: 既存のリポジトリは、ruleset の必須の status check を `pre-merge-check` に置き換える必要がある。

### Confirmation

`pre-merge-check` に `if: always()` があることを、ast-grep の `yaml/workflow-pre-merge-check-always` で検査する。また、`pre-merge-check` が `pre-merge-check.yaml` の外にないことを、`yaml/workflow-pre-merge-check-file` で検査する。`needs` に書き忘れたジョブと ruleset の設定は、レビューで確認する。

## Pros and Cons of the Options

### a. `pre-merge-check` ジョブの `needs` に集め、それだけを必須の status check にする

- Good: リポジトリの設定は `pre-merge-check` を1つ登録するだけで済み、どのリポジトリでも同じになる。
- Bad: スキップを成功として扱う GitHub の仕様に合わせて、`if: always()` と結果の判定を書く必要がある。

### b. ジョブを1つずつ必須の status check に登録する

- Good: ワークフローに判定のためのジョブを書かずに済む。
- Bad: ジョブを加えるたびにリポジトリの設定を変える必要があり、登録し忘れても気付けない。

[^control-jobs-with-conditions]: GitHub の文書によれば、スキップされたジョブの状態は「Success」と報告され、必須の status check であっても PR のマージを止めない。

[^workflow-syntax]: GitHub の文書によれば、ジョブが失敗するかスキップされると、それを `needs` に書いたジョブは、続けて実行する条件を書かない限りスキップされる。

[^troubleshooting-required-status-checks]: パスやブランチのフィルター、コミットメッセージによってワークフローがスキップされると、その status check は「Pending」のままになる。
