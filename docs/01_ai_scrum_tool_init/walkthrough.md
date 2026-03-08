# AIスクラムツール 初期化・DB確認 (Walkthrough)

## 実装内容
1. **プロジェクトの初期化**: `create-tauri-app` を使用し、React + TypeScript環境でTauriプロジェクト一式を `ai-scrum-tool` ディレクトリ内に生成しました。
2. **依存関係の追加**:
    - フロントエンド側に `@tauri-apps/plugin-sql` パッケージを追加
    - バックエンド側（Rust）に `tauri-plugin-sql = { version = "2", features = ["sqlite"] }` を追加
3. **Rustバックエンドの設定**:
    - `src-tauri/Cargo.toml` および `src-tauri/tauri.conf.json` のアプリ名を `ai-scrum-tool` に正しく修正しました。
    - `src-tauri/capabilities/default.json` にて `sql:default` 権限を許可し、フロントエンドからDB操作を行えるようにしました。
    - `src-tauri/src/lib.rs` にSQLiteプラグインの初期化と、マイグレーション機能（DBファイル名: `ai-scrum.db`）の登録コードを組み込みました。
4. **マイグレーション処理の実装**:
    - 指定されたスキーマ要件に基づき、`stories` と `tasks` の2つのテーブルを自動作成する初期化用SQLを `src-tauri/migrations/1_init.sql` として配置しました。

## 確認事項とテスト結果（Manual Verification Request）

AIアシスタントがコマンドを実行する環境では `cargo`（Rustビルドツール）コマンドのパスが解決できず、単独での `npm run tauri dev` の起動および検証が行えませんでした。

そのため、お手数ですが**ユーザー様のローカル環境にて以下をご自身でご確認いただけますでしょうか**。

### 手順
1. **Rust環境のセットアップ (未インストールの場合)**
   現在 `cargo` コマンドが見つからないエラーが発生しています。TauriのバックエンドビルドにはRust言語環境およびC++ビルドツールが必要です。以下の公式サイトから `rustup-init.exe` をダウンロードし、インストールしてください。
   - [Install Rust](https://www.rust-lang.org/tools/install)
   - インストール完了後、**ターミナル（またはVS Code）を再起動**して、`cargo --version` が実行できる状態にしてください。
2. **Visual Studio C++ Build Toolsのインストール (link.exeエラー対策)**
   RustのWindows環境向けビルドには、MicrosoftのC++リンカー（link.exe）が必要です。以下のいずれかの方法でインストールを行ってください。
   - **winget を使う場合 (推奨/ターミナルからおこなえます)**:
     PowerShell等で以下のコマンドを実行します。
     ```bash
     winget install Microsoft.VisualStudio.2022.BuildTools --custom "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
     ```
   - **手動でインストールする場合**:
     [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) からインストーラーをダウンロード・実行し、「**C++によるデスクトップ開発 (Desktop development with C++)**」のワークロードにチェックを入れてインストールしてください。
   - **※インストール完了後は、必ずVS Codeまたはターミナル全体をいったん再起動してください。**
3. ターミナルで `c:\Users\green\Documents\workspaces\ai-scrum-tool` へ移動します。
4. 以下のコマンドを実行してください。
   ```bash
   npm run tauri dev
   ```
4. 初回ビルドに少し時間がかかりますが、エラーなくコンパイルされ、デスクトップアプリケーションのウィンドウが起動することを確認してください。
5. ローカルアプリのデータフォルダ（通常 `%APPDATA%\com.green.ai-scrum-tool` 、 `%LOCALAPPDATA%\com.green.ai-scrum-tool` 、または `C:\Users\green\AppData\Local\com.green.ai-scrum-tool` 等）を確認し、`ai-scrum.db` が自動生成されていることを確認してください。
6. （任意）DB Viewer等で該当ファイルを開き、`stories`・`tasks` テーブルが存在していることをご確認ください。

問題なく起動・DB生成が行われた場合は、当フェーズ（初期化およびDBセットアップ）は完了となります。再度エラー等が発生した場合は共有をお願いいたします。
