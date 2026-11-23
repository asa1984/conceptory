---
type: ADR
title: 規約の配布物を npm パッケージ、Nix flake、conceptory CLI で配る
description: ADR 0018 のうち設定を置くパッケージを定めた部分を置き換える。Node.js のツールが読む設定は npm パッケージ @asa1984/conceptory にまとめて公開し、言語に依存しないツールとルールは Nix flake で配り、スタイルガイドの語彙は conceptory CLI に埋め込む。
tags:
  - distribution
  - npm
  - nix
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-15T04:40:00+09:00
sources:
  - id: node-type-stripping
    resource: https://nodejs.org/api/typescript.html#type-stripping-in-dependencies
    title: "Node.js: Modules: TypeScript"
  - id: pnpm-publish-config
    resource: https://pnpm.io/package_json#publishconfig
    title: "pnpm: package.json publishConfig"
---

# 規約の配布物を npm パッケージ、Nix flake、conceptory CLI で配る

## Context and Problem Statement

conceptory は下流のリポジトリが従う規約を配布物で示す（ADR 0006「横断の規約は配布物で示し ADR には決定の理由を記録する」）。4つのリポジトリ（asa1984.dev、qexil-poc、konstruktsiya、investment）で conceptory の運用を始めたところ、配布物ごとに取り込み方が異なり、次の問題が起きた。

- 下流は conceptory CLI と ast-grep のルールを Nix flake の input から取り込み、`@conceptory/tsconfig` を pnpm の catalog に書いた GitHub の参照から取り込んでいる。conceptory の同じリビジョンを2つのロックファイルで固定しているが、両者をそろえる仕組みはない。
- `@conceptory/vite-plus-config` は TypeScript のファイルをそのまま export している。Node.js は `node_modules` の中の TypeScript から型を取り除かないので[^node-type-stripping]、下流はこのパッケージを import できない。そのため各リポジトリは内容を `config/` に写しており、4つのリポジトリのすべてで写した内容が conceptory とずれている。
- textlint の設定も各リポジトリに写しており、ルール集の5つのパッケージを各リポジトリが依存に並べている。
- スタイルガイドの `vocabulary` は conceptory の `guides/` にしかないので、下流が `conceptory okf vocabulary` で生成する辞書にはスタイルガイドの避ける語が入らない。つまり下流では ADR 0005「日本語の文章を静的検査と LLM の見直しの二段階で検査する」の静的検査がスタイルガイドの辞書を使えていない。

配布物をどのような方法で配れば、下流が写さずに参照でき、更新にも追従できるか。

## Decision Drivers

- 下流が配布物を写さずに参照できること
- 下流がロックファイルで配布物のバージョンを固定し、Renovate で更新できること
- TypeScript を使わないリポジトリに Node.js を求めないこと
- 下流が導入のために書く設定を少なくできること

## Considered Options

- a. Node.js のツールが読む設定は npm パッケージ、言語に依存しないツールとルールは Nix flake、スタイルガイドの語彙は conceptory CLI で配る
- b. すべてを Nix flake で配る
- c. すべてを npm で配り、conceptory CLI もプラットフォームごとのバイナリのパッケージで配る
- d. いまのまま GitHub の参照と各リポジトリへの複製で配る

## Decision Outcome

採用した選択肢: **a**

配布物はそれを読むツールが前提とする方法で配る。TypeScript、Vite+、textlint はどれも設定を `node_modules` から読むので、これらの設定は npm パッケージで配る。conceptory CLI と ast-grep のルールは言語に依存せず Rust や Python だけのリポジトリでも使うので、ADR 0011「ast-grep のルールを言語ごとに分けて Nix flake で配布する」のとおり Nix flake で配る。スタイルガイドの語彙は conceptory CLI だけが読むので、CLI に埋め込む。

`packages/tsconfig` と `packages/vite-plus-config` は `packages/conceptory` にまとめ、`@asa1984/conceptory` として npm のレジストリに公開する。パッケージは次の export を持つ。

| export                    | 中身                                                                                                     |
| ------------------------- | -------------------------------------------------------------------------------------------------------- |
| `./tsconfig/base.json` 他 | ADR 0018「TypeScript のベースラインを定め直し、tsconfig を base と mixin の組み合わせにする」の tsconfig |
| `./lint`                  | oxlint の設定                                                                                            |
| `./fmt`                   | oxfmt の設定                                                                                             |
| `./test`                  | Vitest の設定                                                                                            |
| `./preset-textlint`       | textlint のプリセット                                                                                    |

tsconfig は JSON のまま公開する。Vite+ と textlint の設定は TypeScript で書き、公開するときに `vp pack` で JavaScript と型定義に変換する。conceptory の中では workspace のリンクを通して TypeScript のファイルを直接読み込み、公開する tarball でだけ pnpm の `publishConfig` で export 先を変換後のファイルに切り替える[^pnpm-publish-config]。変換が必要なので GitHub の参照では配れず、npm のレジストリに公開する。リリースの手順は別の ADR で定める。

textlint の設定はルール集をまとめたプリセットとして配る。textlint は ESM の設定ファイルを読まないので、パッケージの設定を読み込む `.textlintrc` は CommonJS で書く必要がある。しかし ADR 0018 はモジュールを Native ESM で書くと定めており、設定ファイルも CommonJS では書かない。一方でプリセットは textlint が ESM のモジュールとして読み込むので、ESM で書ける。

`@asa1984/conceptory` は textlint のルール集を依存に持ち、それらを import して1つにまとめたプリセットを `./preset-textlint` として export する。プリセットの中のルール集はパッケージ自身の依存から解決されるので、pnpm が依存の依存を下流の `node_modules` の直下に置かなくても見つかる。textlint は `@` で始まり `/preset-` を含む名前だけをプリセットとして扱う。そのため export の名前は `./textlint` ではなく `./preset-textlint` にする。下流は `.textlintrc.yaml` でこのプリセットを指定する。

```yaml
filters:
  comments: true
rules:
  "@asa1984/conceptory/preset-textlint": true
```

プリセットは conceptory CLI が生成する辞書を `.textlint/prh.yaml` と `.textlint/vocabulary.json` から読む。プリセットの中のルールの ID は `@asa1984/conceptory/preset-textlint/ja-technical-writing/max-kanji-continuous-len` のようにプリセットの名前を前置きした形になり、`textlint-disable` のコメントにもこの ID を書く。また下流がプリセットの中のルールに書いた設定を、textlint はプリセットの設定とマージしないでそのまま使う。そのため ai-words-ja のルールに `allows` を加える下流は、プリセットが設定した `severity` と `dictionaryPath` もあわせて書く。

フィルタはプリセットに含められないので、下流は textlint-filter-rule-comments を自分の依存に入れる。textlint は下流が起動するので、`@asa1984/conceptory` は textlint を peerDependencies に持つ。下流はこのほかに `@asa1984/conceptory` とその peerDependencies（`vite-plus`、`textlint`）を devDependencies に入れる。

conceptory の flake は CI で使うツールの集合を `packages.ci` として定義し、CI はそれを `nix profile add .#ci` で入れる。devShell はこの集合に開発でだけ使うツールを加えたものにする。下流の flake も同じ名前で CI のツールを定義する。nixpkgs と overlay を下流に提供するかどうかは別の ADR で定める。

`guides/` のスタイルガイドの `vocabulary` は conceptory CLI に埋め込む。`conceptory okf vocabulary` は埋め込んだ語彙を、渡された OKF bundle の語彙に加えて辞書を生成する。conceptory 自身も `guides/` を bundle として渡さず、埋め込んだ語彙を使う。こうすると下流と conceptory が同じ方法で辞書を生成できる。

skill の配り方はこの ADR では定めない。Renovate のプリセットは ADR 0013「依存の更新は Renovate の共有プリセットで配布し、Dependabot は脆弱性のアラートだけに使う」で定めたまま変えない。

### Consequences

- Good: 下流は設定を写さずに `extends` と import で参照し、Renovate で更新できる。
- Good: 下流はルール集を依存に並べる必要がなくなり、textlint の設定も標準の `.textlintrc.yaml` に書ける。
- Good: 下流の辞書にもスタイルガイドの避ける語が入る。
- Good: TypeScript を使わないリポジトリは Nix flake だけで conceptory CLI と ast-grep のルールを使える。
- Bad: npm に公開するので、変換とリリースの仕組みを保守する必要がある。
- Bad: 下流は npm パッケージと flake の input の2か所で conceptory を固定するので、両者のバージョンはずれうる。
- Bad: `textlint-disable` のコメントに書くルールの ID がプリセットの名前の分だけ長くなる。
- Bad: プリセットの中のルールの設定を一部だけ変えるときも、下流はそのルールの設定をすべて書く必要がある。
- Bad: スタイルガイドの語彙を変えたときは、conceptory CLI を更新するまで下流の辞書に反映されない。

### Confirmation

conceptory の CI は `pnpm pack` で作った tarball を一時的なプロジェクトに入れ、tsconfig、Vite+ の設定、textlint のプリセットが `node_modules` から読めることを検査する。生成した辞書が `guides/` のスタイルガイドの語彙を含むことは conceptory CLI のテストで検査する。

## Pros and Cons of the Options

### a. Node.js のツールが読む設定は npm パッケージ、言語に依存しないツールとルールは Nix flake、スタイルガイドの語彙は conceptory CLI で配る

- Good: どの配布物もそれを読むツールが前提とする方法で参照できる。
- Good: Node.js の設定は pnpm のロックファイルで、ツールは flake のロックファイルで固定でき、どちらも Renovate で更新できる。
- Bad: 配布の方法が複数あり、下流が conceptory を固定する箇所も2つになる。

### b. すべてを Nix flake で配る

- Good: 下流が conceptory を固定する箇所が flake のロックファイルだけになる。
- Bad: Node.js は Nix store へのリンクの実体のパスから依存を解決するので、store に置いた Vite+ の設定は下流の `vite-plus` を見つけられない。
- Bad: 設定の更新が npm の依存の更新と別になり、tsconfig を使うだけのリポジトリにも Nix が必要になる。

### c. すべてを npm で配り、conceptory CLI もプラットフォームごとのバイナリのパッケージで配る

- Good: 下流が conceptory を固定する箇所が pnpm のロックファイルだけになる。
- Bad: プラットフォームごとのパッケージを公開し続ける必要があり、Nix でビルドするものとは別にバイナリを保守することになる。
- Bad: TypeScript を使わないリポジトリにも Node.js と pnpm が必要になる。

### d. いまのまま GitHub の参照と各リポジトリへの複製で配る

- Good: 公開の仕組みを用意する必要がない。
- Bad: 写した設定が conceptory とずれ続け、ルール集の依存も各リポジトリに並び続ける。
- Bad: TypeScript のまま export した設定は GitHub の参照で取り込んでも import できない。

## More Information

textlint 15.8 の振る舞いは次のとおりだった。`"type": "module"` のパッケージに置いた `.textlintrc.js` が `export default` で設定を返しても、textlint はルールを1つも読み込まなかった。同じ設定を `.textlintrc.cjs` の `module.exports` で返すと読み込んだ。一方で ESM のプリセットは読み込めた。pnpm で入れたパッケージのサブパスから ESM のプリセットを export して `.textlintrc.yaml` で指定すると、プリセットはパッケージ自身の依存から import したルール集で検査できた。export の名前を `./textlint` にすると、textlint はそれを1つのルールとみなして読み込みに失敗した。`textlint-disable` のコメントにルール集の名前から始まる短い ID を書いてもルールは無効にならず、プリセットの名前を前置きした ID を書くと無効になった。

[^node-type-stripping]: Node.js は `node_modules` の下にある TypeScript のファイルを実行しようとすると `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING` で失敗する。Node.js 26.12 でも TypeScript のファイルを export するパッケージを `node_modules` から import するとこのエラーになった。

[^pnpm-publish-config]: pnpm は `publishConfig` に書いた `exports` などのフィールドを、tarball を作るときに package.json の同じフィールドへ上書きする。
