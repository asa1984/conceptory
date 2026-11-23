---
type: ADR
title: conceptory の flake から overlay を適用した nixpkgs と flake-parts のモジュールを提供する
description: 下流は conceptory の flake が overlay を適用した nixpkgs をそのまま使い、conceptory CLI と横断のツールを conceptory と同じ derivation で取得する。devShell と CI のツールの定義は flake-parts のモジュールにまとめる。
tags:
  - distribution
  - nix
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-15T05:32:00+09:00
---

# conceptory の flake から overlay を適用した nixpkgs と flake-parts のモジュールを提供する

## Context and Problem Statement

ADR 0019「規約の配布物を npm パッケージ、Nix flake、conceptory CLI で配る」により、conceptory CLI と ast-grep のルールは Nix flake で配る。下流はこれまで conceptory の flake を input に加え、その `packages` を自分の nixpkgs と組み合わせて使ってきた。しかし運用の中で次の問題が起きた。

- 下流は conceptory の input の nixpkgs を自分の nixpkgs にそろえていないので、`flake.lock` に nixpkgs と rust-overlay が2つずつ入り、評価する nixpkgs も2つになる。
- 下流の CI と開発者のマシンは、conceptory を更新するたびに conceptory CLI の Rust と viewer をソースからビルドしている。conceptory の CI は Magic Nix Cache でビルドの結果を保存しているが、GitHub Actions のキャッシュはリポジトリごとに分かれるので下流からは使えない。
- devShell と CI のツールの定義を各リポジトリが個別に書いており、pnpm を corepack で入れるリポジトリと nixpkgs の pnpm を使うリポジトリが混在している。nixpkgs がまだ取り込んでいないバージョンのツールを使いたいときも、各リポジトリで対応する必要がある。
- ast-grep のルールへのリンクを置く shellHook と CI の手順を、各リポジトリが写している。

下流が conceptory の配るツールをビルドせずに使い、ツールのバージョンを conceptory でまとめてそろえるには、conceptory の flake から何を提供すべきか。

## Decision Drivers

- 下流が conceptory CLI をソースからビルドせずに取得できること
- 下流が使う横断のツールのバージョンを conceptory でまとめて更新できること
- 下流が導入のために書く Nix の設定が少ないこと
- 下流が必要なら conceptory と異なる nixpkgs を使えること

## Considered Options

- a. overlay を適用した nixpkgs を提供し、devShell と CI のツールを flake-parts のモジュールにまとめる
- b. overlay だけを提供し、下流は自分の nixpkgs に適用する
- c. いまのまま `packages` だけを提供する

## Decision Outcome

採用した選択肢: **a**

conceptory の flake は conceptory が使う nixpkgs に overlay を適用したものを `legacyPackages.${system}` として提供する。下流は自分で nixpkgs を import せず、この `legacyPackages` を pkgs として使う。derivation の出力のパスは nixpkgs と overlay が同じなら同じになるので、下流は conceptory の CI がビルドしてバイナリキャッシュに置いたものをそのまま取得できる。下流が自分の nixpkgs に overlay を適用すると conceptory CLI の出力のパスが変わり、キャッシュに当たらない。バイナリキャッシュの置き方は別の ADR で定める。

`overlays.default` は rust-overlay の overlay と conceptory の overlay をまとめたものである。conceptory の overlay は conceptory CLI と ast-grep のルールに加えて、nixpkgs がまだ取り込んでいないバージョンのツール（新しい Node.js など）を追加する。overlay は nixpkgs の既存の属性を置き換えず、新しい名前で追加する。たとえば `nodejs` を置き換えると、それに依存する nixpkgs のパッケージがすべてリビルドされ、cache.nixos.org のキャッシュに当たらなくなる。nixpkgs より新しい Node.js は、ソースからビルドせずに Node.js の公式のバイナリから作る。

`flakeModules.default` は flake-parts のモジュールであり、次のことをまとめて行う。

- perSystem の pkgs を conceptory の `legacyPackages` にする。
- conceptory CLI と ast-grep のほか、横断で使うツール（actionlint、zizmor、nixfmt など）を `packages.ci` に入れる。
- `packages.ci` のツールに開発でだけ使うツールを加えた devShell を `devShells.default` として定義する。
- devShell の shellHook で、ast-grep のルールへのリンクを ADR 0011「ast-grep のルールを言語ごとに分けて Nix flake で配布する」の例のとおり `.conceptory/ast-grep` に置く。

下流はモジュールを import し、自分だけが使うツールをモジュールのオプションで `packages.ci` と devShell に加える。ADR 0019 で定めた `packages.ci` は、このモジュールが定義する。

```nix
{
  inputs = {
    flake-parts.url = "github:hercules-ci/flake-parts";
    conceptory.url = "github:asa1984/conceptory";
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [ inputs.conceptory.flakeModules.default ];
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      perSystem =
        { pkgs, ... }:
        {
          conceptory.ciPackages = [ pkgs.uv ];
        };
    };
}
```

conceptory と異なる nixpkgs が必要な下流は、モジュールを使わずに自分の nixpkgs に `overlays.default` を適用できる。その場合の下流は conceptory CLI をソースからビルドする。

### Consequences

- Good: 下流は conceptory CLI をソースからビルドせずに、バイナリキャッシュから取得できる。
- Good: nixpkgs と横断のツールの更新は、conceptory の更新として下流に届く。
- Good: 下流の flake は input と自分だけが使うツールを書けばよい。
- Bad: nixpkgs の更新がすべての下流に同時に届くので、nixpkgs の変更が下流を壊したときの影響が広い。
- Bad: 下流は conceptory を更新しないと nixpkgs を更新できない。
- Bad: 公式のバイナリから作る Node.js は、バージョンを上げるたびに conceptory でハッシュを更新する必要がある。
- Bad: flake-parts を使わない下流はモジュールを使えない。

### Confirmation

conceptory の `checks` で、モジュールを import した最小の flake を評価し、`packages.ci` と devShell をビルドできることを検査する。overlay が追加する属性の名前が overlay を適用する前の nixpkgs にないことも `checks` で検査する。

## Pros and Cons of the Options

### a. overlay を適用した nixpkgs を提供し、devShell と CI のツールを flake-parts のモジュールにまとめる

- Good: 下流の derivation が conceptory の CI でビルドしたものと同じになり、バイナリキャッシュに当たる。
- Good: 横断のツールの定義を conceptory の1か所で保守できる。
- Bad: 下流の nixpkgs のバージョンが conceptory の更新に縛られる。

### b. overlay だけを提供し、下流は自分の nixpkgs に適用する

- Good: 下流は nixpkgs を自分の都合で更新できる。
- Bad: conceptory CLI の出力のパスが下流ごとに変わるので、バイナリキャッシュに当たらない。
- Bad: devShell と CI のツールの定義は各リポジトリに残る。

### c. いまのまま `packages` だけを提供する

- Good: 新しい仕組みを用意する必要がない。
- Bad: 下流の `flake.lock` に nixpkgs が2つ入り、conceptory CLI のビルドも各リポジトリで繰り返される。

## More Information

下流が `inputs.conceptory.inputs.nixpkgs.follows = "nixpkgs"` と書いて conceptory の nixpkgs を自分のものに置き換えると、b と同じく conceptory CLI の出力のパスが変わり、キャッシュに当たらなくなる。
