---
type: ADR
title: 文章の規範をスタイルガイドとして定め skill から参照する
description: 日本語の文章の規範を StyleGuide 文書として guides/ に置き、コーディングエージェント向けの skill はその文書を参照する。
tags:
  - writing
  - skill
  - okf
terms:
  - term: スタイルガイド
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T04:03:12+09:00
sources:
  - id: koyobun
    resource: https://www.bunka.go.jp/seisaku/bunkashingikai/kokugo/hokoku/pdf/93651301_01.pdf
    title: 公用文作成の考え方（建議）
  - id: qiita-70k
    resource: https://nyosegawa.com/posts/qiita-writing-before-after-ai/
    title: 生成AI以前と以後でエンジニアの文章はどう変わったのか
  - id: zaitsu-2023
    resource: https://pmc.ncbi.nlm.nih.gov/articles/PMC10411719/
    title: Distinguishing ChatGPT(-3.5, -4)-generated and human-written papers through Japanese stylometric analysis
  - id: hayashi-2026
    resource: https://www.anlp.jp/proceedings/annual_meeting/2026/pdf_dir/B9-17.pdf
    title: LLM による日本語生成におけるモデル固有表現パターンの分析
  - id: antislop
    resource: https://arxiv.org/abs/2510.15061
    title: Antislop
---

# 文章の規範をスタイルガイドとして定め skill から参照する

## Context and Problem Statement

conceptory が規範を定めるリポジトリの文書の多くはコーディングエージェントが書き、人間が読む。コーディングエージェントの書く日本語には、技術用語の不自然な和語化や直訳調の表現などの癖が出やすい。この癖を防ぐ規範をエージェントへ渡すには、skill として書く必要がある。しかし skill の中だけで書くと、人間の読み手や検査ツールが規範を参照しにくい。文章の規範をどこにどのような形で定めるべきか。

## Decision Drivers

- 人間とコーディングエージェントが同じ規範を読めること
- 規範と、その規範を検査する辞書を同じ場所で管理できること
- 特定のエージェント製品に依存しないこと

## Considered Options

- a. StyleGuide 文書として guides/ に置き、skill から参照する
- b. skill の中にだけ書く
- c. 外部の既存の skill を使う

## Decision Outcome

採用した選択肢: **a**

規範を OKF の文書として OKF bundle に置けば、人間は他の文書と同じように読め、conceptory CLI は frontmatter から検査用の辞書を生成できる。skill は規範を書き写さずに参照するので、規範を変更しても skill との食い違いが生じない。

文章の規範を定める文書を **スタイルガイド** と呼ぶ。スタイルガイドは下流へ配布する規範なので、`guides/` に置く（[ADR 0006](0006-express-norms-as-distributed-artifacts.md)）。スタイルガイドの `type` は `StyleGuide` とし、frontmatter のスキーマは `schemas/style-guide.schema.json` で定める。スタイルガイドは frontmatter の `vocabulary` に避ける語を列挙できる。各項目には避ける語の基本形を `avoid` に、代わりに使う語を `prefer` に書き、必要なら品詞を `pos` と `pos-detail` で絞り込む。conceptory CLI はすべての文書の `vocabulary` を集めて textlint-rule-preset-ai-words-ja の辞書を生成する。辞書の使い方は [ADR 0005](0005-check-japanese-in-two-stages.md) で定める。

skill は Agent Skills の形式で `skills/` に置く。skill の `references/` にはスタイルガイドへの、`assets/` には ADR のテンプレートへのシンボリックリンクを置く。こうすると、skill を単体で取り出してもそれらを読める。skill の配布方法は別の ADR で定める。

`skills/` と `schemas/` は下流のリポジトリへ配布するので、conceptory の外でも通用する内容だけを書く。具体的には、ADR への参照と調査の記録（統計や研究結果の紹介）と conceptory に固有の記述やコマンドを含めない。skill から参照するスタイルガイドとテンプレートも配布物に含まれるので、同じ扱いにする。規範の出典は書いてよい。規則を決める根拠になった調査の結果は ADR に書く。

### Consequences

- Good: 規範と検査の辞書が同じ文書にあるので、規範を変更したときに辞書の更新を忘れにくい。
- Good: skill が規範を書き写さないので、規範と skill の内容が食い違わない。
- Bad: skill をシンボリックリンクに対応しない方法で配布すると、スタイルガイドが欠ける。
- Bad: スタイルガイドは避ける語を例として本文に載せるので、その箇所では textlint の検査を無効にする必要がある。
- Bad: スタイルガイドには調査の結果を書かないので、規則を決めた経緯を知るには ADR を読む必要がある。

### Confirmation

conceptory CLI がスタイルガイドの frontmatter をスキーマで検査し、避ける語の重複と、避ける語が用語として定義されていないことを検査する。skill のシンボリックリンクの参照先が存在することは、CI で確認する。配布物に ADR への参照や調査の記録や conceptory 固有の記述が入っていないことは、PR のレビューで確認する。

## Pros and Cons of the Options

### a. StyleGuide 文書として guides/ に置き、skill から参照する

- Good: 人間とエージェントが同じ文書を読める。
- Good: frontmatter から検査用の辞書を生成できる。
- Bad: skill とスタイルガイドの二か所を管理する必要がある。

### b. skill の中にだけ書く

- Good: 管理する場所が一か所で済む。
- Bad: 人間の読み手が規範を見つけにくい。
- Bad: 検査用の辞書を規範から生成できない。

### c. 外部の既存の skill を使う

- Good: 規範を一から書かずに済む。
- Bad: 既存の skill の方針が conceptory の方針と食い違うことがある。たとえば yomiyasu は硬い漢語を和語に言い換えるよう指示しており、技術文書では分野で慣用の漢語を使うという conceptory の規範と逆の方向を向いている。

## More Information

日本語の文章の規範は [日本語の技術文書の書き方](../../guides/japanese-writing.md) にある。conceptory を使うリポジトリでは、規範と食い違う yomiyasu を無効にする。

<!-- 避ける語を例として挙げるので、この範囲では辞書の検査を無効にする -->
<!-- textlint-disable ai-words-ja/no-ai-words -->

スタイルガイドの各節は、次の規範と調査に基づく。

| 節                             | 根拠                                                                                                                                                                                        |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 修飾の順序と読点               | 本多勝一『日本語の作文技術』の修飾の順序の原則とテンの二大原則。公用文作成の考え方も、長い修飾節から示すよう求めている[^koyobun]。LLM の日本語は読点の位置が人間と異なる[^zaitsu-2023]      |
| 分野で定着した語を使う         | 公用文作成の考え方は、定着した外来語を言い換えずに使うとしている[^koyobun]。「道具」を含む記事は 0.3% から 4.1% に増えた[^qiita-70k]                                                        |
| 語種は読み手と文書の種類で選ぶ | 公用文作成の考え方は、専門的な文書では漢語の方が正確だとしている[^koyobun]。Microsoft の日本語スタイルガイドも、開発者向けの文章では「漢語＋する」の形を勧める                              |
| 直訳調を避ける                 | 公用文作成の考え方の「リスクを取る」の例[^koyobun]、Microsoft の日本語スタイルガイドの逐語訳を避ける規定、安西徹雄『英文翻訳術』。「効く」を含む記事は 2.0% から 22.3% に増えた[^qiita-70k] |
| 文のつながり                   | 木下是雄『理科系の作文技術』、石黒圭『文章は接続詞で決まる』、庵功雄『日本語におけるテキストの結束性の研究』。石黒は接続詞の欠如と多用の両方を戒めている                                    |
| 構文と書式                     | 否定対比の多さ[^antislop]、モデルごとの定型句[^hayashi-2026]、太字やダッシュの増加[^qiita-70k]                                                                                              |

公用文作成の考え方と Microsoft の日本語スタイルガイドは、一般の読み手に向けた文章では和語を勧めている。技術文書で漢語を使うという規範は、両者が専門的な文書や開発者向けの文章を別に扱っていることに基づく。一方で「正本」の和語化と漢語の動詞の和語化については、調査で出典が見つからなかった。この2つは conceptory の文書を書く中での観察に基づく。

<!-- textlint-enable ai-words-ja/no-ai-words -->

[^koyobun]: 文化審議会「公用文作成の考え方（建議）」2022年。

[^qiita-70k]: Qiita の約7万記事を生成 AI の登場前後で比べた集計。査読はなく、著者自身も増加の原因が生成 AI だとは言い切れないとしている。

[^zaitsu-2023]: Zaitsu と Jin による 2023 年の PLOS ONE の論文。機能語の比率や読点の位置などを特徴量として、ChatGPT が書いた日本語の論文と人間が書いた論文を識別した。

[^hayashi-2026]: 林と相澤による、言語処理学会の第32回年次大会での発表。モデルごとの定型句を抽出し、出力からモデルを識別した。

[^antislop]: Paech らによる 2025 年の論文。英語の LLM 出力では「not X, but Y」型の表現が人間の文章よりはるかに多く出ると報告している。
