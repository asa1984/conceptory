---
type: ADR
title: conceptory CLI を Rust で実装し OKF 周りの機能をその1つとして提供する
description: conceptory が配布する規範を扱う CLI を Rust で実装し、OKF bundle の検査と辞書の生成をその機能の1つとして提供する。
tags:
  - okf
  - tooling
  - rust
terms:
  - term: conceptory CLI
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T15:12:03+09:00
sources:
  - id: okf-crate
    resource: https://github.com/W4G1/okf
    title: W4G1/okf
---

# conceptory CLI を Rust で実装し OKF 周りの機能をその1つとして提供する

## Context and Problem Statement

[ADR 0001](0001-write-documents-as-okf-bundle.md) で、ドキュメントを OKF bundle として書くと決めた。conceptory は OKF の上に `deprecated-by` やリンクの書き方などの独自の規則を追加しているので、OKF への適合とあわせてこれらの規則も検査する必要がある。リンク切れや `index.md` との不整合のように、検査の多くはファイル単体ではなく OKF bundle 全体を見なければ判定できない。さらに conceptory は、今後も OKF 以外の規範を扱うツールを配布する。これらの検査や生成をどのようなツールで行うべきか。

## Decision Drivers

- OKF bundle 全体を対象にした検査を書けること
- TypeScript 以外のリポジトリでも Node.js なしで動かせること
- Nix でパッケージ化して配布しやすいこと
- conceptory 独自の規則を中心に、機能を追加していけること

## Considered Options

- a. conceptory 専用の CLI を Rust で実装する
- b. conceptory 専用の CLI を TypeScript で実装する
- c. remark-lint などの既存の Markdown linter に plugin を追加して組み合わせる
- d. 既存の OKF 用のツールを使う

## Decision Outcome

採用した選択肢: **a**

Rust で実装すれば単一のバイナリになるので、Node.js を持たないリポジトリにも Nix でそのまま配布できる。既存の Markdown linter はファイル単位の検査を前提にしており、OKF bundle 全体を見る検査の多くを plugin として書きにくい。既存の OKF 用のツールは OKF の仕様への適合しか検査せず、conceptory 独自の規則と食い違う判定もする。

conceptory が配布する規範を扱う CLI を **conceptory CLI** と呼び、コマンド名は `conceptory` とする。OKF 周りの機能は conceptory CLI の機能の1つであり、`conceptory okf` のサブコマンドにまとめる。

| サブコマンド                | 内容                                              |
| --------------------------- | ------------------------------------------------- |
| `conceptory okf check`      | 1つ以上の OKF bundle を検査する                   |
| `conceptory okf index`      | 各ディレクトリの `index.md` を生成する            |
| `conceptory okf terms`      | 用語の一覧か prh の辞書を生成する                 |
| `conceptory okf vocabulary` | textlint-rule-preset-ai-words-ja の辞書を生成する |

`conceptory okf check` は次の項目を検査する。用語と避ける語の重複は、OKF bundle をまたいで検査する。

| 検査                  | 内容                                                                                                                                       |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| OKF への適合          | 予約されたファイル名以外の Markdown が解析可能な frontmatter と空でない `type` を持ち、`index.md` と `log.md` が仕様どおりの構造であること |
| type ごとのスキーマ   | frontmatter が `schemas/` に置いた type ごとの JSON Schema を満たすこと                                                                    |
| type ごとの本文の構造 | ADR であれば `title` と H1 が一致し、テンプレートの必須の節がそろっていること                                                              |
| パス                  | frontmatter のパスが bundle-relative で書かれ、本文のリンクが相対パスで書かれ、どちらも指す先が存在すること                                |
| index                 | 各ディレクトリの `index.md` が配下の文書をすべて載せ、各文書の `description` と一致する説明を持つこと                                      |

frontmatter のスキーマを JSON Schema で書くのは、実装言語を問わずスキーマを読み書きできるからである。本文の構造は JSON Schema では表せないので、type ごとの検査として実装する。

ソースは Cargo workspace の `crates/` に置き、コマンドの定義と機能を別の crate に分ける。コマンドの定義は `crates/conceptory` に、OKF 周りの機能は `crates/conceptory_okf` に置く。機能を追加するときは `conceptory_<機能名>` の crate を追加して CLI から呼び出すので、ある機能の依存がほかの機能に波及しない。

Rust のコードには厳しい lint を適用する。clippy の `all`、`pedantic`、`nursery`、`cargo` のグループはすべて deny にする。restriction のグループからも、`unwrap_used` や `as_conversions` などを有効にする。コンパイラと clippy は stable を使い、rustfmt だけ nightly を使う。rustfmt の import の整理などの設定が nightly でしか使えないからである。下流リポジトリへの配布方法は別の ADR で定める。

### Consequences

- Good: 単一のバイナリなので、下流リポジトリは言語やランタイムを問わず conceptory CLI を使える。
- Good: OKF bundle 全体を一度に読み込むので、ファイルをまたぐ検査を素直に書ける。
- Good: 機能ごとに crate が分かれるので、機能を追加しても既存の機能の依存が増えない。
- Bad: conceptory の中で TypeScript と Rust の2つのツールチェーンを管理する必要がある。
- Bad: rustfmt だけ nightly を使うので、Rust のツールチェーンを2つのバージョンで管理する必要がある。
- Bad: textlint のルールとはコードを共有できないので、textlint と連携するときはファイルを介して受け渡す。

### Confirmation

conceptory の CI で `conceptory okf check` を `docs/` と `guides/` に対して実行する。conceptory CLI 自身は、規則ごとに適合する OKF bundle と違反する OKF bundle を fixture として用意してテストする。clippy と rustfmt の検査も CI で実行する。

## Pros and Cons of the Options

### a. conceptory 専用の CLI を Rust で実装する

- Good: 単一のバイナリになり、Nix でのパッケージ化と配布が容易である。
- Good: Markdown のパーサーと JSON Schema の検証器に維持されているライブラリがある。
- Bad: conceptory に2つ目の言語が加わる。

### b. conceptory 専用の CLI を TypeScript で実装する

- Good: remark の生態系をそのまま使える。
- Good: conceptory の既存のツールチェーンである Vite+ に乗れる。
- Bad: 配布物に Node.js の実行環境を含める必要がある。

### c. remark-lint などの既存の Markdown linter に plugin を追加して組み合わせる

- Good: リンク検査など既存の plugin を再利用できる。
- Bad: ファイルをまたぐ検査を plugin として書きにくい。
- Bad: frontmatter を JSON Schema で検査する remark-lint の plugin は 2023 年から更新されていない。

### d. 既存の OKF 用のツールを使う

- Good: OKF の仕様への適合の検査を既存の実装に任せられる。
- Bad: 本文の bundle-relative リンクを正しいと判定するなど、conceptory 独自の規則と食い違う判定をする。
- Bad: 検査結果に行番号と規則の識別子がなく、conceptory 独自の検査と同じ形式で報告できない。
- Bad: `index.md` の生成と frontmatter の整形が conceptory の書式と両立しない。

## More Information

2026-10-10 に、Rust 製の OKF 用ツールである W4G1/okf の 0.2.7（crate の okf と okf-validator）を評価した[^okf-crate]。conceptory の OKF bundle は OKF v0.2 に適合すると判定されたが、conceptory 独自の規則は検査されず、選択肢 d の Bad に挙げた食い違いがあった。置き換えられる部分は OKF の仕様への適合の検査だけであり、依存先としても個人が保守する 0.x の crate なので採用しなかった。行番号と規則の識別子が検査結果に付いた時点で、改めて評価する。

コマンド名に `okf` を使わないのは、この名前の OKF 用ツールが crates.io、PyPI、GitHub にすでに複数あるからである。conceptory CLI は OKF の仕様にない規則を検査するので、OKF の汎用ツールと誤解される名前も避けた。

用語の検査は [ADR 0003](0003-define-terms-in-frontmatter.md) で定める。conceptory CLI はその検査で使う辞書も生成する。

[^okf-crate]: W4G1/okf は Apache-2.0 で公開されている。YAML と Markdown のパーサーを自前で実装しており、検査結果は重大度とパスとメッセージだけを持つ。
