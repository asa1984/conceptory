---
type: ADR
title: conceptory は CalVer でリリースし、npm パッケージは trusted publishing で公開する
description: conceptory のバージョンは YYYY.M.N の CalVer とし、互換性を約束しない。手動で起動したワークフローがタグと GitHub Release を作り、@asa1984/conceptory を npm の trusted publishing で公開する。
tags:
  - release
  - npm
  - ci
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-24T10:00:00+09:00
sources:
  - id: semver
    resource: https://semver.org/
    title: Semantic Versioning 2.0.0
  - id: generated-release-notes
    resource: https://docs.github.com/en/repositories/releasing-projects-on-github/automatically-generated-release-notes
    title: "GitHub Docs: Automatically generated release notes"
  - id: npm-trusted-publishers
    resource: https://docs.npmjs.com/trusted-publishers
    title: "npm Docs: Trusted publishing for npm packages"
  - id: immutable-releases
    resource: https://docs.github.com/en/code-security/supply-chain-security/understanding-your-software-supply-chain/immutable-releases
    title: "GitHub Docs: Immutable releases"
  - id: release-please
    resource: https://github.com/googleapis/release-please
    title: googleapis/release-please
---

# conceptory は CalVer でリリースし、npm パッケージは trusted publishing で公開する

## Context and Problem Statement

ADR 0019「規約の配布物を npm パッケージ、Nix flake、conceptory CLI で配る」により、`@asa1984/conceptory` を npm のレジストリに公開する。リリースの手順はその ADR では定めなかった。下流は npm パッケージと flake の input の2か所で conceptory を固定するので、両者のバージョンがずれうる。

conceptory が配るのは規約なので、lint のルールを1つ加えるだけで下流の CI が失敗しうる。SemVer に従うと、どの変更が破壊的かを判断する基準が曖昧になり、ほとんどのリリースがメジャーバージョンの更新になる。一方で下流が conceptory を更新する PR は、pre-merge-check を通らなければ Renovate が自動ではマージしない。また npm の公開用のトークンを CI に置くと、トークンが漏れたときに誰でも公開できるようになる。

conceptory のバージョンをどう決め、リリースと npm への公開をどのように行うべきか。

## Decision Drivers

- リリースのための手作業と仕組みが少ないこと
- 変更が破壊的かどうかを判断せずにリリースできること
- npm パッケージと flake の input を同じバージョンで固定できること
- 長期間有効な公開用のトークンを CI に置かないこと

## Considered Options

- a. CalVer の `YYYY.M.N` とし、手動で起動したワークフローがリリースする
- b. SemVer とし、release-please がリリースの PR を作る
- c. 0.x の SemVer の形のまま、release-please で毎回マイナーバージョンを上げる

## Decision Outcome

採用した選択肢: **a**

conceptory のバージョンは `YYYY.M.N` とする。`YYYY` はリリースした年、`M` は先頭に0を付けない月、`N` はその月の中で0から数えたリリースの通し番号である。日付は Asia/Tokyo で数える。同じ日に2回リリースしても `N` が1つ増えるだけで、`2026.11.3` の次は `2026.11.4` になる。日付をそのまま `2026.11.24` とする形は使わない。SemVer の形式は数を3つまでしか持てず、プレリリースの接尾辞を付けると元のバージョンより前に並び、ビルドのメタデータは並び順の比較で無視される[^semver]。そのため同じ日の2回目に付けるバージョンがない。

バージョンは互換性を約束しない。下流が conceptory の更新を受け入れてよいかは、下流の CI が判定する。Renovate はバージョンの差を年が変われば major、月が変われば minor の更新として扱うが、この区別にも意味はない。下流の Renovate のプリセットで conceptory の更新をどう扱うかは、下流への展開のときに決める。

バージョンはリポジトリのファイルに書かない。`Cargo.toml` の `workspace.package.version` と `packages/conceptory/package.json` の `version` は `0.0.0` のままにする。npm パッケージのバージョンは、リリースのワークフローが `pnpm pack` の前に設定する。conceptory CLI は、flake がビルドするときに渡すコミットのリビジョンと日付を `--version` で表示する。下流は npm パッケージを `2026.11.3`、flake の input を `github:asa1984/conceptory/v2026.11.3` のように同じバージョンで固定できる。

リリースは `.github/workflows/release.yaml` を `workflow_dispatch` で起動して行い、main ブランチ以外では実行しない。ワークフローは次の順に進む。

1. 既存の `vYYYY.M.*` のタグから今月の次の `N` を決める。
2. main ブランチの先頭のコミットにタグ `vYYYY.M.N` を付け、GitHub Release を作る。リリースノートは、前のタグからマージした PR のタイトルを GitHub が並べたものを使う[^generated-release-notes]。PR のタイトルは Conventional Commits で書いているので、そのまま変更の一覧になる。
3. `@asa1984/conceptory` のバージョンを設定し、`pnpm pack` で `publishConfig` を適用した tarball を作り、それを `npm publish` で公開する。

ワークフローには concurrency group を設定し、2つの実行が同じ `N` を選ばないようにする。タグがすでにあればタグの作成が失敗するので、同じバージョンを2回公開することはない。npm への公開が失敗したときは、失敗したジョブだけを再実行する。

npm の認証には trusted publishing を使う。npm は GitHub Actions が発行する OIDC のトークンで公開を認めるので、公開用のトークンを CI に置かない[^npm-trusted-publishers]。公開するジョブは `id-token: write` の権限を持ち、main ブランチからしか使えない environment `npm` で実行する。npmjs.com の trusted publisher の設定にはリポジトリ、ワークフローのファイル名、この environment の名前を登録する。npm は trusted publishing で公開したパッケージに provenance を自動で付ける。npm は Nix から入れたものを使い、trusted publishing が求める 11.5.1 以上にする。

パッケージの設定では「Require two-factor authentication and disallow tokens」を選び、トークンによる公開を禁じる。この設定は trusted publishing による公開には影響しない。npm に存在しないパッケージには trusted publisher を設定できないので、最初のバージョンだけは手元から2要素認証で公開し、その後に trusted publisher を設定する。

リポジトリでは immutable releases を有効にする。公開したリリースのタグは移動と削除ができなくなるので[^immutable-releases]、タグで固定した flake の input の中身が後から変わらない。リリースするコミットは main ブランチにあるので、ADR 0021「conceptory の CI がビルドしたパッケージを Cloudflare R2 のバイナリキャッシュで配る」のとおりバイナリキャッシュにも置かれている。

### Consequences

- Good: 変更が破壊的かどうかを判断せずにリリースでき、リリースのための手作業はワークフローを起動することだけになる。
- Good: バージョンを書き換えるコミットとリリースの PR がないので、GitHub App とファイルを書き換える設定が必要ない。
- Good: バージョンを見れば、いつの conceptory かが分かる。
- Good: 公開用のトークンを CI と npm のどちらにも持たないので、漏れて悪用されるトークンがない。
- Bad: バージョンから互換性を読み取れないので、下流は更新のたびに CI で確認する必要がある。
- Bad: CHANGELOG はリポジトリに残らず、GitHub Release のリリースノートだけになる。
- Bad: flake から渡すリビジョンが変わるので、文書だけを変えたコミットでも conceptory CLI がビルドし直される。
- Bad: 最初のバージョンは手元から公開する必要がある。

### Confirmation

リリースのワークフローは、`pnpm pack` で作った tarball の `package.json` のバージョンがタグと一致することを検査してから公開する。ワークフローは ADR 0014「GitHub Actions のアクションはコミットの SHA で固定し、ワークフローを actionlint と zizmor で検査する」のとおり actionlint と zizmor で検査する。

## Pros and Cons of the Options

### a. CalVer の `YYYY.M.N` とし、手動で起動したワークフローがリリースする

- Good: リリースの仕組みがタグを付けて公開するワークフローだけで済む。
- Bad: バージョンが互換性を表さない。

### b. SemVer とし、release-please がリリースの PR を作る

- Good: バージョンで破壊的な変更を伝えられ、リリースの内容を PR としてレビューできる。
- Bad: 規約の変更のどこからが破壊的かの基準が曖昧である。
- Bad: リリースの PR で pre-merge-check を実行するための GitHub App と、Cargo の workspace のバージョンを書き換える設定が必要になる。

### c. 0.x の SemVer の形のまま、release-please で毎回マイナーバージョンを上げる

- Good: 破壊的かどうかを判断せずに、release-please をそのまま使える。
- Bad: SemVer の形をしているが、バージョンが互換性を表さない。
- Bad: GitHub App や Cargo のファイルを書き換える設定が b と同じく必要になる。

## More Information

release-please 17.11.2 のソースを確認した。バージョンの決め方は SemVer にもとづくものだけで、CalVer には対応していない。Rust 向けの戦略はルートの `Cargo.toml` に `[package]` がないと失敗するので、各クレートが `version.workspace = true` でバージョンを継承する conceptory の workspace には使えない。

下流の flake の input をタグで固定したとき、Renovate の nix の manager が flake.nix のタグを更新できるかは確認していない。下流の固定の方法は下流への展開のときに決める。

[^semver]: SemVer 2.0.0 では、プレリリースのバージョンは同じ数のリリースより前に並び、ビルドのメタデータはバージョンの優先順位の判定で無視される。

[^generated-release-notes]: GitHub は前のリリースからマージした PR とその作成者を並べたリリースノートを生成できる。

[^npm-trusted-publishers]: npm の trusted publishing は npm CLI 11.5.1 以上と Node.js 22.14.0 以上を必要とし、GitHub がホストするランナーでだけ使える。公開のリポジトリから公開したパッケージには provenance を自動で付ける。

[^immutable-releases]: immutable releases を有効にすると、公開したリリースのタグは別のコミットに移せず、リリースがある間は削除もできない。リリースを削除しても同じタグの名前は再利用できない。
