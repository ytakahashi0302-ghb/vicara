# AIスクラムツール 実装計画 (MVP基盤初期化およびDBセットアップ)

## 目標
Tauri (Rust) + React (TypeScript) + SQLiteによるローカル動作のAIスクラムツールMVP基盤を初期化し、指定されたスキーマに基づくマイグレーション処理を実装する。

## User Review Required
- **プロジェクトのディレクトリ名と配置場所**: 現在の計画では、`c:\Users\green\Documents\workspaces\ai-scrum-tool` に新規作成する想定ですが、このディレクトリ名と場所でよろしいでしょうか？（異なるディレクトリや現在の `TrueBite` 内への作成をご希望の場合はご指定ください）
- テスト方針や以下の初期化手順に問題がないかご確認ください。

## Proposed Changes
プロジェクト全体を新規に構築します。

### ベースプロジェクト
#### [NEW] `create-tauri-app`によるプロジェクト一式の生成
- フロントエンド: React + TypeScript環境でTauriプロジェクトを生成します。

### Backend (Rust/Tauri)
#### [MODIFY] `src-tauri/Cargo.toml`
- `tauri-plugin-sql` クレートを追加し、`features = ["sqlite"]` を設定します。

#### [NEW] `src-tauri/migrations/1_init.sql`
- 提示された要件に基づく `stories` テーブルと `tasks` テーブルを作成するためのSQLiteスキーマ定義を記載します。

#### [MODIFY] `src-tauri/src/lib.rs` (または `main.rs`)
- Tauri Builderにて、`tauri_plugin_sql::Builder::default().add_migrations(...)` を用いてSQLiteプラグインの初期化と自動マイグレーション処理を追加します。

#### [MODIFY] `src-tauri/capabilities/default.json` (または `tauri.conf.json`)
- SQLプラグインへのアクセス権限（Capabilities）を追加し、フロントエンドからDB操作を可能に設定します。

### Frontend (React/TypeScript)
#### [MODIFY] `package.json`
- `@tauri-apps/plugin-sql` をフロントエンド側の依存パッケージとして追加・インストールします。

## Verification Plan
### Automated Tests
MVPs作成の初期フェーズであり、現在のスコープに自動テストの構築は含まれません。

### Manual Verification
1. `npm run tauri dev` コマンドでデスクトップアプリがエラーなく起動することを確認します。
2. アプリの起動後、指定したパス（例: アプリのデータディレクトリ等）にSQLiteのデータベースファイルが自動生成されていることを確認します。
3. 生成されたデータベースファイルに対してSQLiteクライアントツール等から接続し、`stories` と `tasks` の2つのテーブルが存在し、カラム定義が要件と一致していることを確認します。
