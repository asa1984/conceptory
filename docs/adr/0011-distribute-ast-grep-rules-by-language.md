---
type: ADR
title: ast-grep のルールを言語ごとに分けて Nix flake で配布する
description: ast-grep のルールは言語ごとのディレクトリに置いて Nix flake で配布し、各リポジトリは devShell が置くリンクを通して使う言語のルールを参照する。
tags:
  - ast-grep
  - lint
  - distribution
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-12T18:00:00+09:00
sources:
  - id: ast-grep-config
    resource: https://ast-grep.github.io/reference/sgconfig.html
    title: ast-grep sgconfig.yml reference
---

# ast-grep のルールを言語ごとに分けて Nix flake で配布する

## Context and Problem Statement

conceptory は、lint で表せない規約を ast-grep のルールで検査する。最初のルールは TypeScript 向けに書き、npm の workspace パッケージとして置いた。しかし Rust など他の言語のルールも定める予定であり、npm で配ると Rust だけのリポジトリにも Node.js が必要になる。言語ごとのルールをどのように置き、下流のリポジトリへどのように配るべきか。

## Decision Drivers

- 言語に依存しない方法で配れること
- 各リポジトリが使う言語のルールだけを選べること
- エディタの ast-grep の言語サーバーで、ルールの違反をその場で表示できること
- 言語をまたいでルールの ID が衝突しないこと

## Considered Options

- a. Nix flake で配り、devShell がルールへのリンクを置く
- b. 言語ごとに npm パッケージで配る
- c. conceptory CLI にルールを埋め込み、ast-grep のライブラリで検査する
- d. 各リポジトリにルールを複製する

## Decision Outcome

採用した選択肢: **a**

ルールは `ast-grep/rules/<言語>/` に置き、ルールのテストは `ast-grep/rule-tests/<言語>/` に置く。ルールの ID は `<言語>/<名前>` の形にする[^ast-grep-config]。ast-grep の ID は、走査するすべてのルールの中で一意でなければならない。言語の名前を前置きすれば、言語をまたいで同じ名前のルールを作っても衝突しない。

ast-grep の1つのルールは1つの言語しか対象にできない。TypeScript のルールは、同じファイルに TypeScript と TSX の2つの記述を置き、ID をそれぞれ `typescript/<名前>` と `tsx/<名前>` にする。2つの記述が ID と言語のほかは一致することを、`vp test` のテストで検査する。

Nix flake の `packages.ast-grep-rules` は、言語ごとのディレクトリをそのまま出力する。下流のリポジトリは、devShell の shellHook でこのパッケージへのシンボリックリンクを置き、`sgconfig.yaml` から使う言語のディレクトリを指す。リンクを置くディレクトリは `.gitignore` に加える。

```nix
devShells.default = pkgs.mkShell {
  shellHook = ''
    mkdir -p .conceptory
    ln -sfn ${conceptory.packages.${system}.ast-grep-rules} .conceptory/ast-grep
  '';
};
```

```yaml
ruleDirs:
  - .conceptory/ast-grep/typescript
  - .conceptory/ast-grep/rust
```

conceptory 自身は、編集したルールをすぐに試せるよう、`sgconfig.yaml` から `ast-grep/rules/` を直接指す。

### Consequences

- Good: Node.js のないリポジトリでも、Nix と direnv だけでルールを使える。
- Good: ルールは普通のファイルとして参照されるので、エディタの ast-grep の言語サーバーがそのまま使える。
- Good: 各リポジトリは、使う言語のディレクトリだけを `sgconfig.yaml` に書けばよい。
- Bad: 下流のリポジトリは、shellHook と `.gitignore` の設定を自分で書く必要がある。
- Bad: TypeScript のルールは TypeScript と TSX の2つの記述を保守する必要がある。
- Bad: ID に `/` を含めると、ast-grep はスナップショットを `__snapshots__/<言語>/` に置くが、そのディレクトリを自動では作らない。言語ごとのディレクトリを先に作っておく必要がある。

### Confirmation

`ast-grep test` でルールの振る舞いを、`vp test` で TypeScript と TSX の記述の一致を検査する。Nix の `checks.ast-grep` でも `ast-grep test` を実行する。

## Pros and Cons of the Options

### a. Nix flake で配り、devShell がルールへのリンクを置く

- Good: 言語に依存せず、conceptory CLI などの他のツールと同じ方法で配れる。
- Bad: 下流のリポジトリにリンクを置く設定が必要になる。

### b. 言語ごとに npm パッケージで配る

- Good: TypeScript のリポジトリでは `node_modules` から参照できる。
- Bad: TypeScript 以外のリポジトリにも Node.js が必要になる。

### c. conceptory CLI にルールを埋め込み、ast-grep のライブラリで検査する

- Good: ルールが CLI と同じバージョンで配られ、パスを設定する必要がない。
- Bad: エディタの ast-grep の言語サーバーや `ast-grep test` が使えず、検査結果の報告や抑止のコメントの処理も作り直す必要がある。

### d. 各リポジトリにルールを複製する

- Good: 配布の仕組みを用意する必要がない。
- Bad: 更新が漏れ、リポジトリごとにルールがずれていく。

[^ast-grep-config]: ast-grep は `sgconfig.yaml` の `ruleDirs` に並べたディレクトリからルールを読み込む。ID に `/` を含めても、抑止のコメントの `ast-grep-ignore` で ID を指定できる。
