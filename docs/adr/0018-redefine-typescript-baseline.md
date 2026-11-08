---
type: ADR
title: TypeScript のベースラインを定め直し、tsconfig を base と mixin の組み合わせにする
description: ADR 0009 を置き換える。どの tsconfig も移植性の高い base.json を先頭に継承し、実行環境や役割の差を mixin として extends の配列で加える。oxlint、oxfmt、ast-grep の規約は ADR 0009 から引き継ぐ。
tags:
  - typescript
  - lint
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-08T16:30:00+09:00
sources:
  - id: typescript-5
    resource: https://devblogs.microsoft.com/typescript/announcing-typescript-5-0/
    title: Announcing TypeScript 5.0
  - id: typescript-6
    resource: https://devblogs.microsoft.com/typescript/announcing-typescript-6-0/
    title: Announcing TypeScript 6.0
  - id: oxlint-categories
    resource: https://oxc.rs/docs/guide/usage/linter/rules
    title: Oxlint rules
---

# TypeScript のベースラインを定め直し、tsconfig を base と mixin の組み合わせにする

## Context and Problem Statement

ADR 0009「TypeScript のベースラインを tsconfig、oxlint、oxfmt、ast-grep で定める」は、tsconfig を用途ごとのプリセットに分けた。Node.js 向けの `node.json` とバンドラー向けの `bundler.json` は `base.json` を継承する。公開ライブラリ向けの `library.json` は `node.json` を継承する。しかし使ってみると、この分け方には次の問題があった。

- `library.json` が `node.json` を継承するので、バンドラー向けのライブラリには使えない。
- `bundler.json` が DOM の型と `react-jsx` を含むので、DOM のない実行環境や React を使わないプロジェクトに合わない。
- ブラウザと Node.js のどちらでも動くピュアなパッケージに合う設定がない。
- `types` の配列は `extends` でマージされないので、プリセットが書いた `types` に型を加えるパッケージは継承した値を書き直す必要がある。
- `skipLibCheck` がないので、依存先の型定義まで厳しい設定で検査する。

また viewer は `vite.config.ts` と `src` を1つの tsconfig で検査していたので、Node.js で動く `vite.config.ts` でも DOM の型を使えた。ベースラインの tsconfig をどう分け、各パッケージはそれをどう組み合わせるべきか。

## Decision Drivers

- 何も加えない状態が、どの実行環境でも通用する最も厳しい設定であること
- 実行環境と役割（アプリとライブラリの別）を独立に選べること
- 1つのパッケージの中で、実行環境の異なるファイルを別の設定で検査できること
- 設定の組み合わせ方を静的な検査で守らせられること

## Considered Options

- a. `base.json` を移植性の高い設定にし、ほかの差を mixin として `extends` の配列で加える
- b. 実行環境ごとのプリセット（`node.json` と `bundler.json`）が `base.json` を継承し、役割の差を mixin で加える
- c. ADR 0009 のまま、用途ごとのプリセットを継承でつなぐ

## Decision Outcome

採用した選択肢: **a**

前提は ADR 0009 から変えない。tsconfig は TypeScript 7 以降を前提とし、tsc は常に `noEmit` で型検査だけに使って、ビルドとバンドルはほかのツールに任せる。TypeScript 6.0 で `strict` や `noUncheckedSideEffectImports` などの既定値が変わったので[^typescript-6]、既定値で有効になる設定は書かない。既定値で無効な設定のうち、型検査の厳しさに関わるものはすべて有効にする。モジュールは Native ESM を前提とし、相対パスの import には拡張子を書く。そのため、すべての package.json に `"type": "module"` を書く。`"type"` がないと、Node.js やツールはパッケージの `.js` を CommonJS として扱う。Node.js が型を取り除いてそのまま実行できるよう、`erasableSyntaxOnly` も有効にする。

tsconfig と Vite+ の設定は `packages/` 以下の workspace パッケージに置く。tsconfig は `@conceptory/tsconfig` に、Vite+ の `lint` と `fmt` の設定は `@conceptory/vite-plus-config` に置く。npm への公開とパッケージの名前は、配布の方法を決める ADR で改めて決める。ast-grep のルールは、ADR 0011「ast-grep のルールを言語ごとに分けて Nix flake で配布する」に従って `ast-grep/rules/typescript/` に置く。

`@conceptory/tsconfig` は次の設定を提供する。`base.json` 以外は mixin であり、`base.json` を継承しない。

| ファイル       | 設定                                                                      | 使う場面                       |
| -------------- | ------------------------------------------------------------------------- | ------------------------------ |
| `base.json`    | 厳しさに関わる設定、`module: nodenext`、`lib: ["esnext"]`、`skipLibCheck` | すべての tsconfig              |
| `bundler.json` | `module: preserve`、`allowArbitraryExtensions`                            | バンドラーが import を解決する |
| `dom.json`     | `lib: ["esnext", "dom"]`                                                  | ブラウザで動く                 |
| `react.json`   | `jsx: react-jsx`                                                          | React を使う                   |
| `library.json` | `declaration`、`isolatedDeclarations`                                     | ライブラリとして公開する       |

各パッケージの tsconfig は、`extends` の配列の先頭に `base.json` を書き、必要な mixin をその後ろに並べる[^typescript-5]。たとえばブラウザで動く React のアプリは次のように書く。

```json
{
  "extends": [
    "@conceptory/tsconfig/base.json",
    "@conceptory/tsconfig/bundler.json",
    "@conceptory/tsconfig/dom.json",
    "@conceptory/tsconfig/react.json"
  ],
  "compilerOptions": { "types": ["vite-plus/client"] },
  "include": ["src"]
}
```

`base.json` の `module` を `nodenext` にするのは、Node.js の解決の規則がバンドラーの規則より厳しいからである。`nodenext` で通るコードはバンドラーでも動くが、`preserve` は CSS の import や拡張子のない import を通すので、逆は成り立たない。`lib` を書かないと既定の lib に DOM の型が含まれるので、`base.json` では `lib` を `["esnext"]` に絞る。したがって `base.json` だけを継承したパッケージは Node.js の型も DOM の型も持たず、どの実行環境でも動くピュアなパッケージにそのまま合う。ほかのパッケージは、実行環境に合わせて型と lib を加える。`moduleResolution` は `module` から決まるので、`base.json` には書かない。こうすると `bundler.json` は `module` を上書きするだけで済む。

プリセットには `types` を書かない。プリセットが `types` を書くと、型を加えるパッケージは継承した値を書き直すことになる。どの `@types` を読み込むかは、それを依存に持つパッケージが決めることでもある。そのため Node.js で動くパッケージは、自分の tsconfig に `"types": ["node"]` を書く。

`skipLibCheck` を有効にするのは、厳しさに関わる設定が自分のコードを検査するためのものだからである。`exactOptionalPropertyTypes` などの設定で依存先の型定義を検査すると、依存先の修正を待つしかないエラーが出る。また、依存が増えるほど型検査も遅くなる。`skipLibCheck` は、TypeScript 6.0 で既定値が変わった設定に含まれない。

実行環境の異なるファイルを含むパッケージでは、tsconfig を実行環境ごとに分ける。パッケージの直下の `tsconfig.json` は `"files": []` と `references` だけを持ち、実行環境ごとの tsconfig を参照する。たとえば viewer は、`src` を `tsconfig.app.json` で、`vite.config.ts` を `tsconfig.node.json` で検査する。1つの tsconfig は1つのプログラムになる。その中のどれか1つのファイルで `node:fs` などを import すると、`@types/node` のグローバルはプログラム全体から見える。そのため、`types` を書かないだけでは実行環境を分けられない。また、どの tsconfig の `include` にも入らないファイルは、oxlint の型検査がベースラインではない既定の設定で検査する。そのため各パッケージは自分の tsconfig を持ち、自分のファイルをすべて `include` に含める。

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

- Good: `base.json` だけでピュアなパッケージの設定になり、実行環境と役割を独立に組み合わせられる。
- Good: 1つのパッケージの中でも、Node.js で動くファイルとブラウザで動くファイルを別の設定で検査できる。
- Good: リポジトリが変わっても、同じ厳しさと同じ書き方の規約で TypeScript を書ける。
- Good: 規約違反は検査で見つかるので、レビューで指摘し合う必要がない。
- Good: oxlint の更新で増えたルールを、スナップショットの差分として確認してから受け入れられる。
- Bad: どの tsconfig も `base.json` を書く必要があり、mixin を並べる分だけ `extends` が長くなる。
- Bad: `base.json` の lib は、Node.js とブラウザの両方にある `console` や `URL` も宣言しない。これらを使うピュアなパッケージは型を加える必要がある。
- Bad: `skipLibCheck` は自分のパッケージの `.d.ts` も検査しないので、直接書いた型定義の誤りに気付きにくい。
- Bad: 規約が厳しいので、ライブラリの型定義が緩い箇所では無効化のコメントが必要になる。

### Confirmation

`vp check` は oxlint、oxfmt、型検査をまとめて実行し、`ast-grep scan` は ast-grep のルールで検査する。tsconfig の `extends` の先頭が `base.json` であることは、ast-grep のルール `json/tsconfig-extends-base` で検査する。package.json の `"type"` は、ast-grep のルール `json/package-type-module` で検査する。ast-grep のルールそのものは `ast-grep test` で、oxlint のカテゴリのルールの一覧は `vp test` のスナップショットテストで検査する。

## Pros and Cons of the Options

### a. `base.json` を移植性の高い設定にし、ほかの差を mixin として `extends` の配列で加える

- Good: 何も加えない状態が、どの実行環境でも通用する設定になる。
- Good: 実行環境と役割を独立に選べる。
- Bad: 先頭の `base.json` を書き忘れても TypeScript はエラーにしないので、静的な検査が別に必要になる。

### b. 実行環境ごとのプリセット（`node.json` と `bundler.json`）が `base.json` を継承し、役割の差を mixin で加える

- Good: `node.json` か `bundler.json` を1つ継承すれば、厳しさに関わる設定も含まれる。
- Bad: `base.json` は `module` を持たないので単独では使えず、ピュアなパッケージも `node.json` を継承することになる。そうすると、`node.json` という名前がそのパッケージを Node.js 専用に見せる。

### c. ADR 0009 のまま、用途ごとのプリセットを継承でつなぐ

- Good: 用途に合うファイルを1つ継承すればよい。
- Bad: 継承が一本の線になるので、バンドラー向けのライブラリのように用途を組み合わせられない。
- Bad: プリセットの `types` や `lib` に値を加えるには、継承した値ごと書き直す必要がある。

## More Information

設定を自前で定めて配布する理由と、`@tsconfig/strictest` などの既存の共有設定やリポジトリごとの設定を退けた理由は ADR 0009 に記録してあり、この ADR もそれを引き継ぐ。

TypeScript には、Node.js とブラウザの両方にある API だけを宣言する lib がない。ピュアなパッケージが `console` や `URL` を使う場合は、`"types": ["node"]` を加えるのがよい。そのうえで、Node.js にしかない `process` や `Buffer` を oxlint の `no-restricted-globals` で禁じる。`node:` の import も `no-restricted-imports` で禁じる。DOM の lib を加える方法もあるが、その場合は `document` や `window` や `localStorage` など、禁じるべきグローバルが多すぎる。

[^typescript-5]: `extends` に配列を書けるのは TypeScript 5.0 からである。後ろに書いた設定ほど優先され、`lib` や `types` のような配列の値はマージされずに上書きされる。

[^typescript-6]: TypeScript 6.0 では `strict` と `noUncheckedSideEffectImports` の既定値が `true` に、`types` の既定値が空の配列に変わった。また `dom.iterable` と `dom.asynciterable` は `dom` に統合された。

[^oxlint-categories]: oxlint は、ルールを correctness、suspicious、pedantic、perf、style、restriction、nursery のカテゴリに分けている。nursery は開発中のルールを集めたカテゴリである。
