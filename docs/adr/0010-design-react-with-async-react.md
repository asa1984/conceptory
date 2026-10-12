---
type: ADR
title: React のアプリケーションは Async React と React Compiler を前提に設計する
description: 遷移とデータの更新を transition で行い、データは use と Suspense で読む。メモ化は React Compiler に任せ、エフェクトはクリーンアップできる外部との同期だけに使う。
tags:
  - typescript
  - react
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-12T15:30:00+09:00
sources:
  - id: async-react
    resource: https://github.com/reactwg/async-react
    title: reactwg/async-react
  - id: navigation-api
    resource: https://developer.mozilla.org/en-US/docs/Web/API/Navigation_API
    title: Navigation API
  - id: react-compiler
    resource: https://react.dev/learn/react-compiler
    title: React Compiler
  - id: useeffect-extremist
    resource: https://zenn.dev/uhyo/articles/useeffect-taught-by-extremist
    title: 過激派が教える！　useEffectの正しい使い方
---

# React のアプリケーションは Async React と React Compiler を前提に設計する

## Context and Problem Statement

React 19 は、非同期の処理を React の中で扱う仕組みとして transition、`use`、Suspense、`useOptimistic` を備えている。しかしデータの取得を `useEffect` で行ったり、外部のストアの更新を `useSyncExternalStore` で受けたりすると、取得のたびに読み込み中の表示がちらつく。取得の順序の誤りも起きやすい。`useMemo` や `useCallback` によるメモ化も、依存配列の不足や過剰を生みやすい。React のアプリケーションをどのような設計で書くべきか。

## Decision Drivers

- 画面の切り替えやデータの更新のあいだも、表示が崩れたりちらついたりしないこと
- 非同期の処理の順序や中断を、アプリケーションのコードで管理せずに済むこと
- メモ化やエフェクトの誤りを、人間の注意ではなく静的な検査で防げること

## Considered Options

- a. Async React と React Compiler を前提に設計する
- b. エフェクトでデータを取得し、`useMemo` などでメモ化する

## Decision Outcome

採用した選択肢: **a**

非同期の処理を待つ状態の更新は、すべて transition の中で行う[^async-react]。画面の遷移とサーバーから通知されたデータの更新を transition にすると、データを取得し終えるまで前の画面を表示し続け、取得し終えたらまとめて切り替わる。データは `use` と Suspense で読み、エフェクトでは取得しない。`use` に渡す Promise は同じデータに対して同じオブジェクトでなければならないので、キャッシュに保存して返す。`useSyncExternalStore` で受けた更新は transition にならないので、Suspense するデータの更新には使わない。遷移の途中で選んだ項目の強調などは、`useOptimistic` ですぐに反映する。画面の切り替えのアニメーションには `ViewTransition` を使う。

ルーティングにはルーターのライブラリを使うか、Navigation API で自前に実装する[^navigation-api]。Navigation API では、遷移を intercept して URL の更新を transition の中で行う。intercept の handler が返す Promise は、描画した結果を DOM に反映してから解決する。こうすると、ブラウザはスクロールとフォーカスの復元を新しい画面に対して行う。

メモ化は React Compiler に任せる[^react-compiler]。`useMemo`、`useCallback`、`memo` は oxlint の `no-restricted-imports` で禁じる。React Compiler は描画を純粋な処理とみなして変換するので、描画の中でモジュールの可変な状態を読むと、変わった値を読まなくなる。描画で読む状態は React の状態に置くか、変化をキャッシュのキーに含める。React Compiler は、`@vitejs/plugin-react` が組み込む Rust の実装で有効にする。React Compiler が変換できないコードや、React のルールを抑止するコメントは、oxlint の React のルールで検出する。

`useEffect` は、React の外にあるものとの同期にだけ使い、効果を取り消すクリーンアップ関数を必ず返す[^useeffect-extremist]。コンポーネントを取り除いたときに、その影響がすべて消えるようにするためである。クリーンアップできない処理はイベントハンドラーで行う。依存配列は実行の回数を減らす最適化にだけ使い、値の変化に反応するための仕組みとしては使わない。DOM の要素を置いたときに始める処理には、クリーンアップを返す ref コールバックを使う。クリーンアップを返さないエフェクトは ast-grep のルールで検出する。ルーターが遷移の完了をブラウザに伝えるエフェクトのように、取り消す効果がないものは、無効化のコメントに理由を書いて許す。

### Consequences

- Good: 画面の遷移やデータの更新のあいだも、前の画面を表示し続けるので表示がちらつかない。
- Good: 非同期の処理の順序や中断を、React がまとめて扱う。
- Good: メモ化の依存配列を書く必要がなくなる。
- Bad: Navigation API で実装したルーターは、Navigation API に対応したブラウザでしか動かない。
- Bad: Rust で実装された React Compiler は実験的な段階にあり、変換の誤りに当たる可能性がある。
- Bad: 描画で可変な状態を読んではいけないという制約は、静的な検査では見つけきれない。

### Confirmation

`vp check` で oxlint の React のルールを、`ast-grep scan` でエフェクトのクリーンアップを検査する。

## Pros and Cons of the Options

### a. Async React と React Compiler を前提に設計する

- Good: 非同期の処理と表示の切り替えを React に任せられる。
- Bad: transition と Suspense の振る舞いを理解して設計する必要がある。

### b. エフェクトでデータを取得し、`useMemo` などでメモ化する

- Good: 古い React のバージョンでも同じ書き方ができる。
- Bad: 読み込み中の表示のちらつきや取得の順序の誤りを、アプリケーションのコードで防ぐ必要がある。
- Bad: 依存配列の誤りや不要なエフェクトを、レビューで見つけ続ける必要がある。

[^async-react]: Async React は、React Conf 2025 で示された設計の方針である。ルーターは遷移を transition で行い、データの取得は Suspense に対応する。UI のコンポーネントは action を受け取り、transition の中で実行する。

[^navigation-api]: Navigation API は、ページ内の遷移を `navigate` イベントで受け取り、`intercept` で処理を差し替える API である。リンクのクリックや戻る操作も同じイベントで受け取れる。

[^react-compiler]: React Compiler は、コンポーネントとフックをビルドのときに変換し、値と関数のメモ化を自動で行う。

[^useeffect-extremist]: この記事は、React が UI のライブラリでありコンポーネントを単位とすることから、`useEffect` にはクリーンアップが必要だと論じている。依存配列は最適化のためだけに使うとも論じている。
