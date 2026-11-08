---
name: github-actions
description: GitHub Actions のワークフローの規約と使い方。.github/workflows/ のワークフローを追加、変更、レビューするとき、CI のジョブの構成やランナーを決めるとき、CI が遅い原因を調べるとき、ワークフローの検査をリポジトリに導入するときに使う。
---

# github-actions

この skill は GitHub Actions のワークフローの規約と使い方を定める。静的な検査で守らせている規約はここに書かないので、検査の指摘にはそのまま従う。ワークフローを変更したら、次節の検査をすべて実行する。

## 検査を導入する

ワークフローは次のツールで検査する。リポジトリにこれらの検査がなければ、まず導入する。

| ツール         | 検査する内容                                                                                             |
| -------------- | -------------------------------------------------------------------------------------------------------- |
| actionlint     | 構文と式の誤り。`run` のスクリプトは shellcheck で検査する                                               |
| zizmor         | アクションの固定、権限、外部の入力の埋め込み、認証情報の残留のようなセキュリティの問題                   |
| ast-grep       | ジョブとステップの `name` と `description` の禁止、`pre-merge-check` ジョブの書き方。YAML のルールを使う |
| conceptory CLI | ワークフローの `name` がファイル名と同じであること。`conceptory files check` で検査する                  |

ast-grep は、パスを指定しない限り隠しディレクトリを走査しない。そのため、`.github` を明示して走査する。

```sh
actionlint
zizmor .github/workflows
ast-grep scan . .github
conceptory files check
```

リポジトリの設定では、次の2つを有効にする。

- Settings の Actions の「Require actions to be pinned to a full-length commit SHA」。SHA で固定していないアクションを使うワークフローを失敗させる。
- ruleset の「Require status checks to pass」。必須の status check には `pre-merge-check` だけを登録する。

## マージを止めるジョブを pre-merge-check の needs に並べる

PR のマージを止めるジョブは `.github/workflows/pre-merge-check.yaml` に置き、同じファイルの `pre-merge-check` ジョブの `needs` に並べる。マージを止めるジョブを加えたら、必ず `needs` にも加える。`needs` に書いていないジョブは、失敗してもマージを止めない。

`pre-merge-check` は常に実行し、`needs` のジョブが失敗したかキャンセルされたときに失敗させる。スキップされたジョブは、必須の status check であっても成功として扱われるからである。条件によってスキップされた `needs` のジョブは、失敗として扱わない。

```yaml
pre-merge-check:
  if: always()
  needs: [build, test]
  runs-on: ubuntu-slim
  timeout-minutes: 5
  steps:
    - if: contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled')
      run: exit 1
```

`needs` のジョブがさらに別のジョブを `needs` に持つときは、そのジョブも `pre-merge-check` の `needs` に書く。前のジョブが失敗すると後のジョブはスキップされるので、前のジョブの結果を `pre-merge-check` が見なければ、検査しないままマージできてしまう。

`pre-merge-check.yaml` には `paths` のようなフィルターを付けない。ワークフローごとスキップされると、その status check は保留のままになり、マージを止め続ける。変更したファイルによって検査を省きたいときは、ワークフローではなくジョブをスキップする。変更したファイルを判定するジョブを置いて結果を `outputs` で渡し、後のジョブの `if` で参照すればよい。判定には `dorny/paths-filter` のようなアクションを使える。

```yaml
rust:
  needs: [changes]
  if: needs.changes.outputs.rust == 'true'
pre-merge-check:
  if: always()
  needs: [changes, rust]
```

## ランナーを選ぶ

ビルドやテストを伴わない軽い処理は、`ubuntu-slim` で実行する。たとえば、ジョブの結果をまとめる `pre-merge-check`、ラベルやコメントの操作、`gh` や `jq` で済む短いスクリプトがこれに当たる。`ubuntu-slim` は CPU が1つのランナーで、private なリポジトリでは `ubuntu-latest` より安い。

ただし、`ubuntu-slim` には次の制約がある。どれかに当たる処理は `ubuntu-latest` で実行する。

- ジョブは 15 分で打ち切られて失敗する。`timeout-minutes` は 15 以下にする。
- VM ではなく特権のないコンテナで動く。Docker のデーモン、ファイルシステムのマウント、カーネルの機能を使う処理は動かない。
- 入っているツールは最小限である。Git、GitHub CLI、Node.js、Python、jq などはあるが、Java、Go、Ruby、.NET はない。
- CPU が1つなので、ステップを並列に実行しても CPU を使う処理は速くならない。

どのランナーでも、ジョブには `timeout-minutes` を書く。書かなければ、止まったジョブが既定の 360 分まで動き続ける。

## ステップを並列に実行する

1つのジョブの中のステップは、`background: true` で並列に実行できる。準備に時間のかかるジョブを分けると、ジョブごとに準備を繰り返すことになる。準備を共有したいときは、ジョブを分けずにステップを並列に実行する。

`background: true` を付けたステップは、終わるのを待たずに次のステップへ進む。待つときは `wait` に待つステップの `id` を書くか、`wait-all` で実行中のすべてのステップを待つ。止めるときは `cancel` に `id` を書く。

```yaml
steps:
  - id: build-frontend
    run: pnpm run build:frontend
    background: true
  - id: build-backend
    run: pnpm run build:backend
    background: true
  - run: pnpm run lint
  - wait: [build-frontend, build-backend]
  - run: pnpm test
```

```yaml
steps:
  - id: server
    run: pnpm start
    background: true
  - run: pnpm run test:e2e
  - cancel: server
```

並列に実行するステップには、次の性質がある。

- ステップの失敗は、そのステップを待つ `wait` か `wait-all` で報告される。後のステップがその結果を前提にするなら、後のステップより前で待つ。
- `outputs` と環境変数の変更は、待った後でなければ参照できない。
- 待たなかったステップは、ジョブの終わりに暗黙の `wait-all` で待たれる。
- 同時に実行できるのは、1つのジョブにつき 10 ステップまでである。
- composite action の中のステップには `background` を付けられない。ただし、composite action そのものは `background` のステップとして実行できる。

`parallel` にステップを並べると、それぞれに `background: true` を付けて後に `wait` を置いたことになる。ただし、`parallel` を受け付けない環境が報告されているので、`background` と `wait` で書く。

actionlint は、これらの構文にまだ対応していない。使うときは `.github/actionlint.yaml` で次の指摘を抑止する。2つ目の指摘は `run` と `uses` のどちらも持たないステップの誤りと区別できないので、この誤りも検査されなくなる。そのため、`wait` や `cancel` 以外のステップに `run` か `uses` があることをレビューで確認する。

```yaml
paths:
  .github/workflows/*.yaml:
    ignore:
      - 'unexpected key "background" for step'
      - 'step must run script with "run" section or run action with "uses" section'
```
