---
type: ADR
title: 編集を即座に反映する OKF bundle の viewer を conceptory CLI に埋め込む
description: conceptory CLI がローカルの HTTP サーバーを起動し、埋め込んだ React の SPA で OKF bundle を GitHub 互換の表示とリンクのグラフで閲覧させ、ファイルの変更を SSE で即座に反映する。
tags:
  - okf
  - viewer
  - tooling
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-11T02:38:49+09:00
sources:
  - id: comrak
    resource: https://github.com/kivikakk/comrak
    title: kivikakk/comrak
  - id: github-writing
    resource: https://docs.github.com/en/get-started/writing-on-github
    title: Writing on GitHub
  - id: vite-cve
    resource: https://github.com/vitejs/vite/security/advisories/GHSA-vg6x-rcgg-rjx6
    title: "CVE-2025-24010: Vite の dev server の DNS rebinding"
---

# 編集を即座に反映する OKF bundle の viewer を conceptory CLI に埋め込む

## Context and Problem Statement

OKF bundle の文書は Markdown のファイルなので、文書の本文と文書どうしの関係を一度に見渡しにくい。そこで、文書を読みやすく表示し、文書間のリンクをグラフで示す viewer が欲しい。viewer は手元で開くものであり、ファイルの編集はその場で表示へ反映される必要がある。また、文書は GitHub でも読まれるので、viewer の表示は GitHub の Markdown の表示と互換である必要がある。viewer をどのように実装し、どのように配るべきか。

## Decision Drivers

- ファイルの編集が、ページを再読み込みせずに表示へ反映されること
- 表示が GitHub の Markdown と互換であること
- Node.js のない下流のリポジトリでも、conceptory CLI だけで使えること
- 廃止や用語などの conceptory 独自の情報を表示できること

## Considered Options

- a. conceptory CLI がローカルのサーバーを起動し、埋め込んだ SPA に変更を通知する
- b. conceptory CLI が静的な HTML を書き出す
- c. VitePress や ox-content などの既存のドキュメントサイトのツールを使う
- d. 既存の OKF 用の可視化ツールを使う

## Decision Outcome

採用した選択肢: **a**

ファイルの変更を表示に反映するには、ファイルを監視してブラウザに通知する仕組みが必要になる。SPA を conceptory CLI のバイナリに埋め込めば、Node.js のないリポジトリでも同じ viewer を使える。

`conceptory okf serve <roots>...` でローカルの HTTP サーバーを起動し、既定のブラウザで viewer を開く。サーバーは新しい crate の `conceptory_server` に実装し、axum と tokio を使う。ファイルの監視には notify-debouncer-full を使い、短い間隔の変更をまとめて受け取ってから OKF bundle を読み込み直す。読み込み直した結果は文書ごとのハッシュで前回と比べ、変わった文書の識別子を Server-Sent Events でブラウザに通知する。ブラウザはその文書だけを API から取得し直す。通知はサーバーからブラウザへの一方向なので、WebSocket ではなく Server-Sent Events を使う。

サーバーは `127.0.0.1` だけで待ち受け、Host ヘッダーと Origin ヘッダーが自身を指さないリクエストを拒否する[^vite-cve]。API は GET だけを受け付け、文書はファイルシステムを直接開かずに読み込み済みの識別子で引く。

Markdown の解析には comrak を使い、検査と表示で同じパーサーを使う[^comrak]。これまで検査に使っていた pulldown-cmark は、拡張 autolink と見出しのアンカーの生成に対応していない。comrak は GFM の仕様に適合し、GitHub の alert と見出しのアンカーの規則と数式と emoji の shortcode にも対応する[^github-writing]。comrak が出力した HTML は ammonia でサニタイズする。

Rust 側で行えない処理はブラウザ側で行う。Mermaid の図は mermaid.js で、数式は GitHub と同じ MathJax で描画する。コードのシンタックスハイライトには、GitHub と同じ文法を使う starry-night を使い、見た目には github-markdown-css を使う。MathJax のフォントと拡張、starry-night の正規表現エンジンは何も指定しなければ CDN から読み込まれる。これらは viewer の成果物に同梱し、viewer が外部のサーバーと通信しないようにする。

viewer は `viewer/` に置く React の SPA とし、TypeScript で書いて Vite+ でビルドする。React の設計は ADR 0010「React のアプリケーションは Async React と React Compiler を前提に設計する」に従う。見た目は Tailwind CSS で書き、本文には github-markdown-css を使う。画面は Obsidian のように3列に分け、左に文書の一覧、中央に本文、右にグラフとバックリンクを常に表示する。グラフは Cytoscape.js で描き、レイアウトには fcose を使う。表示中の文書とリンクでつながる文書だけを描くローカルのグラフと、すべての文書を描くグラフを切り替えられる。グラフは支援技術で読めないので、バックリンクとリンク先は HTML の一覧でも表示する。

ビルドした SPA は rust-embed で conceptory CLI に埋め込む。Nix では viewer を別の derivation としてビルドし、その成果物を conceptory CLI のビルドに渡す。

### Consequences

- Good: ファイルを保存すると、ページを再読み込みせずに表示が更新される。
- Good: 下流のリポジトリは conceptory CLI だけで viewer を使える。
- Good: 検査と表示が同じパーサーを使うので、見出しやリンクの解釈が食い違わない。
- Bad: conceptory CLI に非同期のランタイムと HTTP サーバーの依存が加わる。
- Bad: conceptory CLI のビルドが viewer のビルドに依存するので、Nix の構成が複雑になる。
- Bad: ブラウザ側で描画する Mermaid と数式とハイライトは、GitHub と細部で一致しないことがある。

### Confirmation

conceptory CLI のテストで、API の応答と変更の通知と Host ヘッダーの検査を確認する。表示の互換性は、GitHub の Markdown の機能を集めた fixture を viewer で開いて確認する。

## Pros and Cons of the Options

### a. conceptory CLI がローカルのサーバーを起動し、埋め込んだ SPA に変更を通知する

- Good: 変更された文書だけを取得し直して表示を更新できる。
- Good: 検査と同じ読み込みの結果をそのまま表示できる。
- Bad: サーバーのセキュリティ対策を自前で行う必要がある。

### b. conceptory CLI が静的な HTML を書き出す

- Good: サーバーが不要で、実装が単純である。
- Bad: ファイルの変更を表示に反映できない。

### c. VitePress や ox-content などの既存のドキュメントサイトのツールを使う

- Good: Markdown の表示と変更の反映が最初からそろっている。
- Bad: Node.js が必要になり、下流のリポジトリに配りにくい。
- Bad: 廃止や用語などの OKF bundle の情報を表示するには、ツールごとの拡張を作る必要がある。

### d. 既存の OKF 用の可視化ツールを使う

- Good: OKF bundle のグラフ表示がすでにある。
- Bad: 調べた範囲のツールは、ファイルの変更の反映と conceptory 独自の情報の表示に対応していない。

## More Information

グラフの描画ライブラリには sigma.js と graphology の組み合わせも検討した。WebGL で描くので数千ノードを超えても速いが、階層のレイアウトとリポジトリ単位の枠（複合ノード）がない。文書の数が数千を超えた場合は、改めて検討する。

[^comrak]: comrak 0.56 は GFM の仕様テストの全例に適合する。0.56 で GFM の tagfilter が削除されたので、サニタイズは別に行う必要がある。

[^github-writing]: GitHub は alert として NOTE、TIP、IMPORTANT、WARNING、CAUTION の5種類を扱い、数式を MathJax で描画する。

[^vite-cve]: Vite の dev server には、Host ヘッダーと WebSocket の Origin を検査していなかったために DNS rebinding で通信を読み取られる脆弱性があった。
