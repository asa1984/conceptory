---
type: ADR
title: Vitest はテストのファイルを分離せずに実行し、状態を残す書き方を静的に検査する
description: Vitest の isolate を無効にしてモジュールの評価を減らし、モジュールのモックやグローバルの直接の書き換えのようにファイルをまたいで状態を残す書き方を oxlint と ast-grep で禁じる。
tags:
  - typescript
  - test
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-12T20:00:00+09:00
sources:
  - id: vitest-performance
    resource: https://vitest.dev/guide/improving-performance
    title: Improving Performance | Vitest
---

# Vitest はテストのファイルを分離せずに実行し、状態を残す書き方を静的に検査する

## Context and Problem Statement

Vitest は、何も設定しなければテストのファイルごとにモジュールを評価し直し、テストのファイルどうしを分離する。多くのファイルが同じモジュールを読み込むと、そのモジュールはファイルの数だけ評価され、テストの実行が遅くなる。分離を無効にすれば速くなる。しかし、あるテストのファイルが残した状態は、別のファイルのテストに影響しうる。テストを速くしつつ、ファイルどうしの影響をどう防ぐべきか。

## Decision Drivers

- テストの実行を速くすること
- テストの結果が、テストのファイルを実行する順序に左右されないこと
- 状態を残す書き方を、人間の注意ではなく静的な検査で防げること

## Considered Options

- a. 分離を無効にし、状態を残す書き方を静的に検査する
- b. 分離を有効のままにする
- c. 分離を無効にし、テストのファイルを実行する順序を毎回変えて影響を見つける

## Decision Outcome

採用した選択肢: **a**

`@conceptory/vite-plus-config/test` の Vitest のベースラインで `isolate: false` にする[^vitest-performance]。分離を無効にすると、共有するモジュールはワーカーごとに1回だけ評価され、テストのファイルをまたいで状態が残る。Vitest はテストのファイルを終えるたびに `vi.restoreAllMocks()` を呼ぶが、モジュールのモックとモジュールの状態は元に戻さない。

そこで、ファイルをまたいで状態を残す書き方を次のように扱う。

- モジュールのモックに使う `vi.mock` などのメソッドは、oxlint の `vitest/no-restricted-vi-methods` で禁じる。禁じるのは `vi.mock`、`vi.doMock`、`vi.unmock`、`vi.doUnmock`、`vi.resetModules` である。モックしたモジュールは後に実行するファイルにも残り、先に読み込まれたモジュールにはモックが適用されない。モックが必要な依存は、引数で受け取る形にする。
- `globalThis` や `process.env` を直接書き換えることは、ast-grep のルールで禁じる。代わりに `vi.stubGlobal` と `vi.stubEnv` を使う。変えた値は、ベースラインの `unstubGlobals` と `unstubEnvs` によってテストのあとで元へ戻る。
- `vi.useFakeTimers()` と `vi.setSystemTime()` を使うテストのファイルには、`vi.useRealTimers()` を書くことを ast-grep のルールで求める。
- スパイは、ベースラインの `restoreMocks` によってテストが終わると元へ戻る。

### Consequences

- Good: 共有するモジュールを評価する回数が減り、テストが速くなる。
- Good: ファイルをまたいで状態を残す典型的な書き方は、静的な検査で見つかる。
- Bad: モジュールのモックが使えないので、依存を引数で受け取る設計が必要になる。
- Bad: テストがモジュールの変数やキャッシュを変える場合、その状態は残る。この影響は静的な検査では見つけきれない。

### Confirmation

モジュールのモックは oxlint の `vitest/no-restricted-vi-methods` で検査する。グローバルの直接の書き換えは ast-grep の `typescript/vitest-no-global-mutation` で検査する。フェイクタイマーの戻し忘れは `typescript/vitest-restore-fake-timers` で検査する。

## Pros and Cons of the Options

### a. 分離を無効にし、状態を残す書き方を静的に検査する

- Good: テストが速くなり、典型的な影響は検査で防げる。
- Bad: モジュールのモックが使えない。

### b. 分離を有効のままにする

- Good: テストのファイルどうしの影響を考えずに済む。
- Bad: 共有するモジュールをファイルの数だけ評価するので、テストが遅い。

### c. 分離を無効にし、テストのファイルを実行する順序を毎回変えて影響を見つける

- Good: 静的な検査で見つけられない影響も、実行して見つけられる。
- Bad: 失敗するかどうかが実行のたびに変わり、原因を突き止めにくい。

[^vitest-performance]: Vitest の文書は、副作用に頼らず状態を片付けるプロジェクトであれば、分離を無効にすると速くなるとしている。
