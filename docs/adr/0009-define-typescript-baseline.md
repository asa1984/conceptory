---
type: ADR
title: TypeScript のベースラインを tsconfig、oxlint、oxfmt、ast-grep で定める
description: TypeScript 7 以降と Native ESM を前提に、型検査の厳しさに関わる設定とモダンな書き方の規約を配布物で定め、静的に検査する。
tags:
  - typescript
  - lint
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-12T13:50:00+09:00
sources:
  - id: typescript-6
    resource: https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/
    title: Announcing TypeScript 6.0
  - id: oxlint-categories
    resource: https://oxc.rs/docs/guide/usage/linter/rules
    title: Oxlint rules
---

# TypeScript のベースラインを tsconfig、oxlint、oxfmt、ast-grep で定める

## Context and Problem Statement

viewer の追加で、このリポジトリにも TypeScript のコードが入った。TypeScript の設定と書き方の規約をリポジトリごとに決めると、厳しさや書き方がリポジトリによって異なる。そうなると、リポジトリを移るたびに人間とエージェントが規約を読み直す必要がある。すべてのリポジトリで使う TypeScript のベースラインをどう定めるか。

## Decision Drivers

- 型検査をできる限り厳しくすること
- 書き方の規約を人間の注意ではなく静的な検査で守らせること
- ツールの更新によるルールの変化に気付けること
- 各リポジトリが設定をそのまま参照できること

## Considered Options

- a. tsconfig、oxlint、oxfmt、ast-grep の設定を自前で定めて配布する
- b. `@tsconfig/strictest` などの既存の共有設定を使う
- c. リポジトリごとに決める

## Decision Outcome

採用した選択肢: **a**

tsconfig と Vite+ の設定は `packages/` 以下の workspace パッケージに置く。tsconfig は `@conceptory/tsconfig` に、Vite+ の `lint` と `fmt` の設定は `@conceptory/vite-plus-config` に置く。npm への公開とパッケージの名前は、配布の方法を決める ADR で改めて決める。ast-grep のルールは、ADR 0011「ast-grep のルールを言語ごとに分けて Nix flake で配布する」に従って `ast-grep/rules/typescript/` に置く。

tsconfig は TypeScript 7 以降を前提とする。TypeScript 6.0 で `strict` や `noUncheckedSideEffectImports` などの既定値が変わったので[^typescript-6]、既定値で有効になる設定は書かない。既定値で無効な設定のうち、型検査の厳しさに関わるものはすべて有効にする。モジュールは Native ESM を前提とし、相対パスの import には拡張子を書く。そのため、すべての package.json に `"type": "module"` を書く。`"type"` がないと、Node.js やツールはパッケージの `.js` を CommonJS として扱う。Node.js が型を取り除いてそのまま実行できるよう、`erasableSyntaxOnly` も有効にする。設定は、Node.js 向けの `node.json`、バンドラー向けの `bundler.json`、公開ライブラリ向けの `library.json` に分ける。tsc は常に `noEmit` で型検査だけに使い、ビルドとバンドルはほかのツールに任せる。公開ライブラリ向けの `library.json` では、ほかのツールが型定義を出力できるよう `isolatedDeclarations` を有効にする。

oxlint のカテゴリのうち、correctness、suspicious、pedantic、perf、nursery はまるごと有効にする[^oxlint-categories]。style と restriction は書き方の好みや特定の機能の禁止を集めたカテゴリなので、ルールを個別に選ぶ。まるごと有効にしたカテゴリでも、方針に合わないルールや誤検知の多いルールは理由を書いて無効にする。Promise を返す関数をすべて async にする `promise-function-async` は使わない。Suspense のキャッシュは同じ Promise を返す必要があり、async にすると呼ぶたびに別の Promise になるからである。oxlint を更新するとカテゴリに含まれるルールが変わるので、有効なカテゴリのルールの一覧をスナップショットテストで記録し、増減を差分として確認してから受け入れる。oxfmt では import の並べ替えを有効にし、import の順序を決定的にする。

書き方の規約は次のとおりである。oxlint で表せない規約は ast-grep のルールで検査する。

- 関数はアロー関数で書く。`function` を使うのは、`this` の束縛やホイスティングが必要な箇所に限り、無効化のコメントに理由を書く。こうすると、`function` が必要な箇所がコードの中で目立つ。
- 型の宣言は `type` で書く。公開ライブラリでは、利用者が宣言のマージで型を拡張できるよう `interface` を許す。
- 型のメンバーはメソッド記法ではなく関数プロパティ記法で書く。メソッド記法は引数の型を双変で検査するので、型の誤りを見逃す。
- クラスのアクセス修飾子は使わず、JavaScript の private element を使う。アクセス修飾子は型検査の段階でしか制限せず、実行時にはどこからでもアクセスできる。
- `index.ts` は置かない。ディレクトリ `x/` のモジュールをまとめて公開するには、`x/` と並べて `x.ts` を置く。`x.ts` は、あとから `x/` に分割しても import する側のパスが変わらない。
- 引数が2つ以上の関数は、引数をオブジェクトで受け取る。呼び出す側のコードで各引数の役割を読めるようにするためである。ライブラリや組み込み関数が位置引数を要求するコールバックは対象外とする。
- ファイル名はケバブケースにする。大文字と小文字を区別しないファイルシステムでも、名前が衝突しない。

### Consequences

- Good: リポジトリが変わっても、同じ厳しさと同じ書き方の規約で TypeScript を書ける。
- Good: 規約違反は検査で見つかるので、レビューで指摘し合う必要がない。
- Good: oxlint の更新で増えたルールを、スナップショットの差分として確認してから受け入れられる。
- Bad: 規約が厳しいので、ライブラリの型定義が緩い箇所では無効化のコメントが必要になる。

### Confirmation

`vp check` は oxlint、oxfmt、型検査をまとめて実行し、`ast-grep scan` は ast-grep のルールで検査する。package.json の `"type"` は、ast-grep のルール `json/package-type-module` で検査する。ast-grep のルールそのものは `ast-grep test` で、oxlint のカテゴリのルールの一覧は `vp test` のスナップショットテストで検査する。

## Pros and Cons of the Options

### a. tsconfig、oxlint、oxfmt、ast-grep の設定を自前で定めて配布する

- Good: Vite+ に同梱された oxlint と oxfmt をそのまま使える。
- Good: oxlint で表せない規約も ast-grep で検査できる。
- Bad: 設定を自分で保守する必要がある。

### b. `@tsconfig/strictest` などの既存の共有設定を使う

- Good: 保守を他者に任せられる。
- Bad: tsconfig しかなく、書き方の規約や lint の設定は別に決める必要がある。
- Bad: TypeScript 7 の既定値を前提にしていない。

### c. リポジトリごとに決める

- Good: リポジトリの事情に合わせられる。
- Bad: 厳しさや書き方がリポジトリによって異なる。

[^typescript-6]: TypeScript 6.0 では `strict` と `noUncheckedSideEffectImports` の既定値が `true` に変わった。`types` の既定値は空の配列になった。

[^oxlint-categories]: oxlint は、ルールを correctness、suspicious、pedantic、perf、style、restriction、nursery のカテゴリに分けている。nursery は開発中のルールを集めたカテゴリである。
