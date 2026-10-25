## 環境

- Nix (flake) + direnv
- Node.js (v26)
- pnpm (v12)
- Vite+
- ast-grep

## コマンド

コマンドは package.json の `scripts` ではなく、各パッケージの `vite.config.ts` に Vite Task のタスクとして定義している。

```sh
vp run check      # フォーマット、lint、型検査を行う
```

## コミットメッセージ

Conventional Commits の形式（`<type>(<scope>): <description>`）で英語で記述する。scope には変更した crate やパッケージの名前を書き、リポジトリ全体に関わる変更では省略する。PR のタイトルも同じ形式で書く。

## コメントの書き方

コード内のコメントは英語で書く。

1. 型、プロパティ、関数などの宣言へのコメントは doc comment（Rust の `///` や TypeScript の `/** ... */`）で書く。
2. Issue や Pull Request の番号を書かない。
3. コードがしていることを同じ抽象度で言い直すだけのコメントを書かない。
4. 同じ説明を複数の場所に書かない。
5. そのモジュールが知る必要のない知識を書かない。
6. ADR などを参照する際、ID や番号だけでなくタイトルも付記する。

## 参照

- ドキュメント一覧: [docs/index.md](docs/index.md)
- ADR: [docs/adr/](docs/adr/index.md)
