---
type: ADR
title: 依存の更新は Renovate の共有プリセットで配布し、Dependabot は脆弱性のアラートだけに使う
description: Renovate のプリセットを renovate/ に置いて各リポジトリから参照させ、Dependabot はバージョンの更新に使わず脆弱性のアラートだけを有効にする。
tags:
  - renovate
  - dependencies
  - distribution
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-12T21:00:00+09:00
sources:
  - id: renovate-presets
    resource: https://docs.renovatebot.com/config-presets/
    title: Shareable Config Presets | Renovate Docs
  - id: dependabot-shared-config
    resource: https://github.com/orgs/community/discussions/151871
    title: Dependabot should support a global configuration for organizations
  - id: dependabot-nix
    resource: https://github.blog/changelog/2026-04-07-dependabot-version-updates-now-support-the-nix-ecosystem/
    title: Dependabot version updates now support the Nix ecosystem
---

# 依存の更新は Renovate の共有プリセットで配布し、Dependabot は脆弱性のアラートだけに使う

## Context and Problem Statement

各リポジトリの依存を更新する設定は、リポジトリごとに書くと内容がずれていく。conceptory で依存の更新の方針をそろえ、各リポジトリへ配りたい。依存を更新するツールには Renovate と Dependabot があり、使う言語や環境はリポジトリごとに異なる。どのツールの設定を、どのような形で配るべきか。

## Decision Drivers

- 1か所で設定を変えれば、すべてのリポジトリに反映されること
- Rust、JavaScript、Nix、GitHub Actions のどのリポジトリでも同じ設定で使えること
- 運用の判断が必要な設定と、広く推奨されている設定を区別すること

## Considered Options

- a. Renovate の共有プリセットを配る
- b. Dependabot の設定を各リポジトリに生成する
- c. Renovate と Dependabot の両方の設定を配る

## Decision Outcome

採用した選択肢: **a**

Renovate はリポジトリにある設定ファイルを共有プリセットとして参照でき、git のタグで固定もできる[^renovate-presets]。Dependabot には設定を共有する仕組みがなく、各リポジトリに設定ファイルを置く必要がある[^dependabot-shared-config]。

プリセットは `renovate/default.json5` に置き、各リポジトリは `default.json5` をタグを付けて参照する。`default.json5` は、Renovate が推奨する設定をまとめた `config:best-practices` を継承する[^best-practices]。運用の判断が必要な設定は、方針を決めてから加える。エコシステムごとの設定はファイルを分け、`default.json5` から相対パスで読み込む。相対パスで読み込んだプリセットには、`default.json5` に付けたタグが引き継がれる。

```json5
{ extends: ["github>asa1984/conceptory//renovate/default.json5#v1.0.0"] }
```

エコシステムごとのルールは、そのエコシステムのファイルがないリポジトリでは何もしない。そのため、言語や環境に応じて配布物を分ける必要はなく、各リポジトリが参照するプリセットは1つで足りる。

運用については次のように決めた。

- 平日は更新を確認できないので、Asia/Tokyo の金曜と土曜に PR を作る。ロックファイルの更新も同じ曜日にそろえる。脆弱性の修正は、Renovate の既定どおりスケジュールに関係なく PR を作る。
- 0.x 以外の依存の minor と patch の更新は、1つの PR にまとめて CI が通れば自動でマージする。0.x の依存は minor でも破壊的な変更を含みうるので、個別の PR で確認する。メジャーアップデートは自動ではマージせず、PR を作る。
- crate も、npm と同じく公開から3日待ってから更新する。
- Nix のマネージャーはベータの段階にある。それでも flake.lock を更新できるよう有効にする。
- 上げてよいバージョンがほかの依存で決まる依存は、Renovate で更新しない。Vite+ は vite のエイリアスと vitest を同梱したバージョンに固定するよう求めるので、これらは `vp migrate` で上げる。実験的な段階にある React Compiler の Rust の実装は、`@vitejs/plugin-react` が要求するバージョンに合わせて一緒に上げる。
- MSRV は互換性の約束なので、ツールチェーンに合わせて自動では上げない。

Dependabot はバージョンの更新には使わず、脆弱性のアラートだけを有効にする。Renovate は GitHub の脆弱性のアラートを受けて、修正の PR を作る。Dependabot は 2026 年に Nix flake のバージョンの更新に対応したが[^dependabot-nix]、設定を共有できない点は変わらない。

conceptory 自身も同じプリセットを `renovate.json5` から参照する。プリセットは Renovate に同梱された `renovate-config-validator` で検査する。

### Consequences

- Good: プリセットを変えると、参照しているすべてのリポジトリの更新の方針が変わる。
- Good: 各リポジトリの設定は、`default.json5` を参照する1行で済む。
- Bad: Dependabot だけで依存を更新しているリポジトリは、Renovate に移る必要がある。
- Bad: Renovate がプリセットを読み込めないと、そのリポジトリの依存は更新されない。
- Bad: 自動でマージする更新の安全は CI に頼るので、CI のないリポジトリでは動かなくなる更新に気付けない。
- Bad: Renovate で更新しない依存は、人間が新しいバージョンを確認し続ける必要がある。

### Confirmation

`renovate-config-validator` でプリセットの書式を検査する。更新のまとまりと対象から外した依存は、Renovate の dry-run で確認する。

## Pros and Cons of the Options

### a. Renovate の共有プリセットを配る

- Good: 設定を1か所で変えられ、git のタグで固定できる。
- Bad: Renovate を GitHub App として導入する必要がある。

### b. Dependabot の設定を各リポジトリに生成する

- Good: GitHub に組み込まれており、別のアプリを導入せずに済む。
- Bad: 設定を共有できないので、各リポジトリの設定ファイルを生成し、配った内容とずれていないかを検査する仕組みが必要になる。

### c. Renovate と Dependabot の両方の設定を配る

- Good: どちらのツールを使うリポジトリにも対応できる。
- Bad: 同じ依存の PR が2つのツールから作られ、2つの設定を保守する必要がある。

## More Information

2026-10-12 に、このリポジトリで Renovate 44.126.1 の dry-run を実行した。Cargo、pnpm の catalog、`rust-toolchain.toml` の依存は、Renovate に組み込まれたマネージャーで拾えた。Nix flake のマネージャーはベータの段階にあり、明示的に有効にしなければ使われない。更新をまとめる設定だけでは、`vitest` を Vite+ に同梱されたバージョンより先に上げる更新を防げなかった。`oxc-transform-react` を `@vitejs/plugin-react` が要求する範囲の外に上げる更新も同様である。そのため、これらの依存は更新の対象から外した。

[^best-practices]: `config:best-practices` は `config:recommended` を継承し、GitHub Actions と Docker のイメージをダイジェストで固定する。また、npm のパッケージは公開から3日待ってから更新し、ロックファイルを週に1回更新する。

[^renovate-presets]: Renovate の文書によれば、タグを付けて参照したプリセットが同じリポジトリのプリセットを読み込むと、読み込まれる側にも同じタグが使われる。

[^dependabot-shared-config]: Dependabot の設定のうち、組織の単位で共有できるのはプライベートなレジストリの設定だけである。

[^dependabot-nix]: Dependabot は flake.lock の入力を監視し、上流に新しいコミットがあると PR を作る。対象はバージョンの更新だけで、セキュリティの更新は含まない。
