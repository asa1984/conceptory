---
type: ADR
title: conceptory の CI がビルドしたパッケージを Cloudflare R2 のバイナリキャッシュで配る
description: conceptory の CI は下流が使うパッケージを3つのシステムでビルドして署名し、Nix 標準の形式で Cloudflare R2 のバケットに置く。下流は独自ドメインで公開したバケットを substituter に加え、conceptory CLI などをビルドせずに取得する。
tags:
  - distribution
  - nix
  - ci
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-11-20T10:00:00+09:00
sources:
  - id: nix-s3-binary-cache-store
    resource: https://nix.dev/manual/nix/2.33/store/types/s3-binary-cache-store
    title: "Nix Reference Manual: S3 Binary Cache Store"
  - id: r2-public-buckets
    resource: https://developers.cloudflare.com/r2/buckets/public-buckets/
    title: "Cloudflare R2: Public buckets"
  - id: r2-pricing
    resource: https://developers.cloudflare.com/r2/pricing/
    title: "Cloudflare R2: Pricing"
  - id: make-a-binary-cache
    resource: https://zenn.dev/asa1984/articles/make-a-binary-cache
    title: バイナリキャッシュを作ろう
---

# conceptory の CI がビルドしたパッケージを Cloudflare R2 のバイナリキャッシュで配る

## Context and Problem Statement

ADR 0020「conceptory の flake から overlay を適用した nixpkgs と flake-parts のモジュールを提供する」により、下流は conceptory が overlay を適用した nixpkgs をそのまま使うので、conceptory CLI などの derivation は下流でも conceptory のものに一致する。しかし derivation が同じでも、ビルドした結果を下流が読める場所に置かなければ下流はソースからビルドする。conceptory の CI は Magic Nix Cache でビルドの結果を保存しているが、GitHub Actions のキャッシュはリポジトリごとに分かれるので下流からは読めない。cache.nixos.org には nixpkgs のパッケージしかないので、conceptory CLI や overlay が追加するパッケージは下流の CI と開発者のマシンがそれぞれビルドしている。

conceptory の CI がビルドしたパッケージを、下流がどこから取得するようにすべきか。

## Decision Drivers

- private なリポジトリの CI も含め、下流が認証なしにキャッシュを読めること
- 特定のサービスの専用のクライアントに依存しないこと
- サーバーを運用しないこと
- conceptory の CI がビルドしたものだけを下流が受け入れること
- 不特定多数が読んでも、料金が R2 の無料枠を大きく超えないこと

## Considered Options

- a. Cloudflare R2 に Nix 標準の形式で置き、独自ドメインの HTTPS で公開する
- b. Cachix を使う
- c. Attic などのキャッシュのサーバーを自分で運用する
- d. キャッシュを用意せず、下流がそれぞれビルドする

## Decision Outcome

採用した選択肢: **a**

conceptory の CI は下流が使うパッケージを Nix 標準のバイナリキャッシュの形式で Cloudflare R2 のバケットに置く。Nix は S3 互換の API を持つストレージに `nix copy` で書き込めるので[^nix-s3-binary-cache-store]、書き込みに専用のクライアントは必要ない。バケットは `cache.asa1984.dev` のドメインで公開し、下流はこの URL を HTTP のバイナリキャッシュとして substituter に加える。R2 が用意する `r2.dev` のドメインはレート制限があり開発用とされているので使わない[^r2-public-buckets]。キャッシュの形式は Nix 標準なので、置き場所を替えるときも中身を移して URL を替えればよい。

キャッシュに置くのは overlay が追加するパッケージと `packages.ci` のクロージャのうち cache.nixos.org から取得できないパスである。`nix copy` は書き込み先にないパスをクロージャからすべて書き込むので、CI は cache.nixos.org にあるパスを除いてから渡す。CI はこれらを x86_64-linux、aarch64-linux、aarch64-darwin のそれぞれのランナーでビルドする。conceptory の CI は自分の `checks` のために Magic Nix Cache を使い続ける。キャッシュは公開されるので、private な下流のビルドの結果は置かない。

CI はキャッシュに置くパスを署名の鍵で署名し、下流は公開鍵を `trusted-public-keys` に加える。Nix は信頼する鍵で署名されていないパスを substituter から受け入れないので、R2 の API トークンが漏れても下流に別の内容を渡すことはできない。キャッシュに書き込むのは main ブランチへの push で実行するジョブだけとし、署名の鍵と R2 の API トークンは main ブランチからしか使えない GitHub Actions の environment に置く。API トークンの権限はこのバケットのオブジェクトの読み書きに絞る。pull request のジョブは鍵を読めないので、レビュー前のコードでビルドしたものは署名されない。

下流は substituter と公開鍵を flake の `nixConfig` に書かない。Nix は flake の `nixConfig` を利用者が受け入れたときだけ適用し、`accept-flake-config` で受け入れるとその flake が任意の設定を変えられるようになる。下流の CI は Nix をインストールするアクションの設定に、開発者は自分の nix.conf（NixOS や nix-darwin ではその設定）に次の2行を加える。

```ini
extra-substituters = https://cache.asa1984.dev
extra-trusted-public-keys = cache.asa1984.dev-2:<公開鍵>
```

キャッシュは誰でも読めるので、R2 に届く読み出しの回数を抑える。R2 は外部への転送量には料金がかからないが、読み出しの操作は月に1000万回を超えると回数に応じて料金がかかる[^r2-pricing]。バケットの `nix-cache-info` には `Priority: 41` を書き、cache.nixos.org（優先度 40）より後に問い合わせさせる。Nix は先に問い合わせた substituter で見つかったパスを後の substituter には問い合わせないので、nixpkgs のパスの問い合わせは R2 に届かない。また `cache.asa1984.dev` には Cloudflare のキャッシュルールを設定し、エッジのキャッシュに当たったリクエストを R2 に届かせない。NAR は同じ URL の中身が変わらないので長くキャッシュし、narinfo は書き込みと削除を反映するために1時間とする。404 はキャッシュしない。キャッシュすると CI が書き込んだ後も下流が古い 404 を受け取るためである。

古いパスは30日で消す。CI はキャッシュへ書き込むたびに、そのリビジョンとシステムで置くべきパスの一覧を manifest としてバケットに置く。一覧には、すでにあったので書き込まなかったパスも含める。週に1回実行するジョブは、30日以内に置いた manifest のどれにも載っていないパスの narinfo を消し、30日より前の manifest も消す。ただしシステムごとの最新の manifest は30日より前でも残す。main への push が30日なかったときに、最新のリビジョンのパスまで消さないためである。NAR は narinfo を消した次の実行で、どの narinfo からも参照されていなければ消す。narinfo を残したまま NAR を消すと、下流は substitute に失敗して止まるためである。1週間あければ、エッジにキャッシュした narinfo も期限が切れている。書き込みのジョブと削除のジョブは同じ concurrency group に入れ、同時には実行しない。

### Consequences

- Good: 下流は conceptory CLI と overlay が追加するパッケージをビルドせずに取得できる。
- Good: 下流は認証なしにキャッシュを読めるので、private なリポジトリの CI でも substituter と公開鍵を書くだけで済む。
- Good: キャッシュの形式が Nix 標準なので、置き場所を替えても下流は URL を替えるだけで済む。
- Good: nixpkgs のパスの問い合わせとエッジのキャッシュに当たった読み出しは R2 に届かないので、読む人が増えても料金はほとんど増えない。
- Bad: R2 のバケット、ドメイン、キャッシュルール、API トークン、署名の鍵を conceptory で管理する必要がある。
- Bad: manifest をもとに古いパスを消すスクリプトを保守する必要がある。
- Bad: 30日より前のリビジョンの conceptory を固定した下流は、キャッシュから消えたパスをソースからビルドする。
- Bad: 下流の開発者は自分の nix.conf を書き換える必要がある。
- Bad: main ブランチへの push のたびに、macOS を含む3つのランナーでビルドする。

### Confirmation

conceptory の CI はキャッシュに書き込んだ後で R2 の認証情報を持たないジョブを実行し、`packages.ci` を `--max-jobs 0` でビルドする。このジョブの substituter は cache.nixos.org と `cache.asa1984.dev` だけにする。`--max-jobs 0` ではローカルでビルドできないので、置き忘れたパスや署名のないパスがあればビルドが失敗する。削除のジョブの後にも同じジョブを実行し、main の最新のリビジョンのパスを消していないことを検査する。

## Pros and Cons of the Options

### a. Cloudflare R2 に Nix 標準の形式で置き、独自ドメインの HTTPS で公開する

- Good: 書き込みと読み出しを Nix の標準の機能で行える。
- Good: 読み出しの転送量に料金がかからない。
- Bad: cache.nixos.org にあるパスを除く処理と古いパスの削除を自分で用意する必要がある。

### b. Cachix を使う

- Good: cache.nixos.org にあるパスを自動で除き、古いパスの削除も Cachix が行う。
- Bad: 書き込みに Cachix のクライアントとアカウントが必要になる。
- Bad: 置き場所を替えるときは、下流の substituter と公開鍵も替える必要がある。

### c. Attic などのキャッシュのサーバーを自分で運用する

- Good: 古いパスの削除とパスをまたいだ重複の除去をサーバーが行う。
- Bad: サーバーとデータベースを運用する必要がある。

### d. キャッシュを用意せず、下流がそれぞれビルドする

- Good: 用意するものがない。
- Bad: 下流のすべての CI と開発者のマシンが conceptory CLI をビルドし続ける。

## More Information

`cache.asa1984.dev` は以前にも R2 のバイナリキャッシュとして公開していた[^make-a-binary-cache]。そのときの署名の鍵 `cache.asa1984.dev-1` は秘密鍵を失ったので、新しい鍵を作って `cache.asa1984.dev-2` とする。末尾の番号は鍵の世代であり、鍵を作り直すたびに増やす。以前に置いたパスは新しい鍵で署名されていないので、下流はそれらを受け入れない。またバケットに古い鍵で署名したパスが残っていると、`nix copy` はそのパスを書き込み済みとみなして新しい鍵で署名し直さない。そのためバケットの中身を消してから使う。

鍵を替えるときは新しい世代の鍵で署名し直し、下流が新しい公開鍵を加えてから古い公開鍵を外す。

R2 の前に Cloudflare Workers を置く構成もあるが、使わない。Workers が必要になるのは、認証をかけるとき、読まれた時刻をもとに古いパスを消すとき、レスポンスを加工するときである。公開のキャッシュを manifest をもとに消す conceptory には、どれも必要ない。読み出しが想定より増えたときは、Workers ではなく Cloudflare のレート制限のルールで IP アドレスごとの回数を制限する。

[^nix-s3-binary-cache-store]: Nix は `s3://` のストアの `endpoint` に S3 互換のサービスのエンドポイントを指定でき、`secret-key` に指定した鍵で書き込むパスに署名する。

[^r2-public-buckets]: Cloudflare は `r2.dev` のドメインでの公開にレート制限をかけており、本番では独自ドメインを使うよう案内している。

[^r2-pricing]: R2 はストレージの容量と操作の回数に料金がかかり、外部への転送量には料金がかからない。読み出しの操作（Class B）は月に1000万回まで無料で、それを超えると100万回あたり0.36ドルかかる。

[^make-a-binary-cache]: この記事は、GitHub Actions でビルドしたパッケージに署名して `nix copy` で R2 に置き、`cache.asa1984.dev` から取得する手順を示している。
