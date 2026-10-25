# ADR

- [意思決定を MADR 4.0 形式の ADR で記録する](0000-use-madr-for-architecture-decisions.md) - リポジトリ横断の意思決定を MADR 4.0 を基にした形式の ADR で記録する。
- [ドキュメントを OKF bundle として書く](0001-write-documents-as-okf-bundle.md) - conceptory と下流リポジトリのドキュメントを Open Knowledge Format v0.2 の OKF bundle として書き、検査ツールで検査する。
- [conceptory CLI を Rust で実装し OKF 周りの機能をその1つとして提供する](0002-implement-conceptory-cli-in-rust.md) - conceptory が配布する規約を扱う CLI を Rust で実装し、OKF bundle の検査と辞書の生成をその機能の1つとして提供する。
- [用語を導入した文書の frontmatter で定義し textlint で表記を検査する](0003-define-terms-in-frontmatter.md) - 用語は導入した文書の frontmatter の terms で定義し、conceptory CLI が生成する prh の辞書を使って textlint で表記ゆれを検査する。
- [文章の規約をスタイルガイドとして定め skill から参照する](0004-define-style-guides-for-skills.md) - 日本語の文章の規約を StyleGuide 文書として guides/ に置き、コーディングエージェント向けの skill はその文書を参照する。
- [日本語の文章を静的検査と LLM の見直しの二段階で検査する](0005-check-japanese-in-two-stages.md) - 日本語の文章は textlint と conceptory CLI による静的検査と LLM による見直しの二段階で検査し、繰り返し見つかる問題は静的検査に移して再発を防ぐ。
- [横断の規約は配布物で示し ADR には決定の理由を記録する](0006-express-norms-as-distributed-artifacts.md) - 下流のリポジトリが従う規約はスタイルガイドや skill などの配布物で示し、ADR は conceptory の意思決定とその理由の記録として docs/adr にまとめる。
- [YAML ファイルの拡張子を .yaml に統一する](0007-use-yaml-extension.md) - YAML ファイルの拡張子は .yaml に統一し、conceptory CLI で .yml のファイルを検出する。ツールが名前を決めて生成するファイルは除く。
- [編集を即座に反映する OKF bundle の viewer を conceptory CLI に埋め込む](0008-embed-live-okf-viewer.md) - conceptory CLI がローカルの HTTP サーバーを起動し、埋め込んだ React の SPA で OKF bundle を GitHub 互換の表示とリンクのグラフで閲覧させ、ファイルの変更を SSE で即座に反映する。
- [TypeScript のベースラインを tsconfig、oxlint、oxfmt、ast-grep で定める](0009-define-typescript-baseline.md) - TypeScript 7 以降と Native ESM を前提に、型検査の厳しさに関わる設定とモダンな書き方の規約を配布物で定め、静的に検査する。
- [React のアプリケーションは Async React と React Compiler を前提に設計する](0010-design-react-with-async-react.md) - 遷移とデータの更新を transition で行い、データは use と Suspense で読む。メモ化は React Compiler に任せ、エフェクトはクリーンアップできる外部との同期だけに使う。
- [ast-grep のルールを言語ごとに分けて Nix flake で配布する](0011-distribute-ast-grep-rules-by-language.md) - ast-grep のルールは言語ごとのディレクトリに置いて Nix flake で配布し、各リポジトリは devShell が置くリンクを通して使う言語のルールを参照する。
- [Vitest はテストのファイルを分離せずに実行し、状態を残す書き方を静的に検査する](0012-run-vitest-without-isolation.md) - Vitest の isolate を無効にしてモジュールの評価を減らし、モジュールのモックやグローバルの直接の書き換えのようにファイルをまたいで状態を残す書き方を oxlint と ast-grep で禁じる。
- [依存の更新は Renovate の共有プリセットで配布し、Dependabot は脆弱性のアラートだけに使う](0013-distribute-renovate-presets.md) - Renovate のプリセットを renovate/ に置いて各リポジトリから参照させ、Dependabot はバージョンの更新に使わず脆弱性のアラートだけを有効にする。
- [GitHub Actions のアクションはコミットの SHA で固定し、ワークフローを actionlint と zizmor で検査する](0014-harden-github-actions.md) - CI は Nix の devShell と flake の検査を GitHub Actions で実行し、アクションの固定と最小の権限を守らせ、ワークフローを静的に検査する。
- [import alias は package.json の imports で定める](0015-use-package-imports-for-import-aliases.md) - パッケージの中のモジュールをエイリアスで import するときは、エイリアスを package.json の imports に "#/*" として定め、tsconfig の paths は使わない。
