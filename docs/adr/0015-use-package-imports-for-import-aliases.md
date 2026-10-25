---
type: ADR
title: import alias は package.json の imports で定める
description: パッケージの中のモジュールをエイリアスで import するときは、エイリアスを package.json の imports に "#/*" として定め、tsconfig の paths は使わない。
tags:
  - typescript
  - module
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-25T14:30:00+09:00
sources:
  - id: node-subpath-imports
    resource: https://nodejs.org/api/packages.html#subpath-imports
    title: "Node.js: Modules: Packages, Subpath imports"
  - id: typescript-6
    resource: https://www.typescriptlang.org/docs/handbook/release-notes/typescript-6-0.html
    title: TypeScript 6.0 release notes
---

# import alias は package.json の imports で定める

## Context and Problem Statement

パッケージの中のモジュールを深い相対パスで import すると、`../../../features/blog.ts` のようにパスが読みにくくなる。そのため `@/features/blog.ts` のようなエイリアスで import することがある。エイリアスは tsconfig の `paths` でも package.json の `imports` でも定められるが、どちらで定めるかによって、エイリアスを解決できるツールが変わる。エイリアスインポートを使うとき、エイリアスをどこで定めるべきか。

## Decision Drivers

- 1か所の定義を、Node.js、TypeScript、バンドラー、テストランナーがすべて読めること
- Node.js が型を取り除いて TypeScript を直接実行するときにも、エイリアスを解決できること
- エイリアスの定め方を静的な検査で守らせられること

## Considered Options

- a. package.json の `imports` に定める
- b. tsconfig の `paths` に定める

## Decision Outcome

採用した選択肢: **a**

エイリアスインポートを使うかどうかは、各パッケージに任せる。使う場合は、エイリアスを package.json の `imports` に定める[^node-subpath-imports]。エイリアスは `"#/*"` の1つにし、パッケージのソースのディレクトリを指す。

```json
{
  "imports": {
    "#/*": "./src/*"
  }
}
```

エイリアスで import するときも、相対パスと同じく拡張子まで書く（`import { get_post } from "#/features/blog.ts"`）。`imports` はディレクトリの `index` を探さず、拡張子も補わない。ディレクトリの名前で import する必要がある場合は、その `index` のファイルを指すエイリアスを `"#/*"` の前に追加する。

tsconfig の `paths` は使わない。`paths` は TypeScript の型検査だけが読む設定であり、Node.js や多くのバンドラーは読まない。そのため同じエイリアスをバンドラーやテストランナーの設定に重ねて書くことになり、定義がずれる。`imports` は Node.js の仕様であり、TypeScript やバンドラーもこれを読む。

`#/` で始まるエイリアスを Node.js が受け付けるのは 25.4.0 と 24.14.0 からであり、TypeScript が解決するのは 6.0 からである[^typescript-6]。どちらも ADR 0009「TypeScript のベースラインを tsconfig、oxlint、oxfmt、ast-grep で定める」の前提に含まれる。

ADR 0009 のとおり tsc には JavaScript を出力させないので、ベースラインの tsconfig では `rewriteRelativeImportExtensions` を使わない。この設定は、tsc が JavaScript を出力するときに import の `.ts` を `.js` に書き換える。書き換えるのは相対パスの import だけなので、この設定を有効にすると、TypeScript はエイリアスに `.ts` を書いた import をエラーにする。

### Consequences

- Good: エイリアスの定義が1か所になり、どのツールでも同じように解決される。
- Good: Node.js で TypeScript を直接実行するときにも、エイリアスを使える。
- Bad: ディレクトリの `index` を import するコードには、エイリアスを個別に追加する必要がある。

### Confirmation

ast-grep のルール `json/no-tsconfig-paths` で、tsconfig に `paths` を書いていないことを検査する。

## Pros and Cons of the Options

### a. package.json の `imports` に定める

- Good: Node.js の仕様なので、ツールごとに設定を書かずに済む。
- Bad: ディレクトリや拡張子を補わないので、エイリアスが指すファイルを正確に書く必要がある。

### b. tsconfig の `paths` に定める

- Good: ディレクトリの `index` や拡張子を補って解決する。
- Bad: TypeScript しか読まないので、バンドラーやテストランナーに同じエイリアスを設定し直す必要がある。

[^node-subpath-imports]: `imports` のエイリアスは `#` で始める必要がある。`*` を使うと、一致した部分を指す先のパスに埋め込める。

[^typescript-6]: TypeScript は 5.4 から package.json の `imports` を読む。`#/` だけで始まるエイリアスを扱えるようになったのは 6.0 からである。
