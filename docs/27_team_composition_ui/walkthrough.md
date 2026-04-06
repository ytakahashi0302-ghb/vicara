# Epic 27: 修正内容の確認

## 概要

Dev チーム構成を管理するためのデータ保存基盤と UI を追加した。
今回の実装により、グローバル設定から以下を管理できるようになった。

- 最大並行稼働数 `max_concurrent_agents`
- Dev チームのロール一覧
- 各ロールの役割名
- 各ロールのシステムプロンプト
- 各ロールに割り当てる Claude モデル

## バックエンド

- `src-tauri/migrations/12_team_configuration.sql` を追加
- `team_settings` テーブルを追加し、`max_concurrent_agents` を保持
- `team_roles` テーブルを追加し、role の可変長リストを保持
- 初回保存エラーを防ぐため、`Lead Engineer` を初期ロールとしてシード投入
- `src-tauri/src/db.rs` に以下を追加
  - `TeamSettings`
  - `TeamRole`
  - `TeamConfiguration`
  - `TeamConfigurationInput`
  - `get_team_configuration`
  - `save_team_configuration`
- `save_team_configuration` はトランザクションで一括更新する構成にした
- サーバー側で以下のバリデーションを実装した
  - `max_concurrent_agents` は `1〜5`
  - `roles` は最低 1 件必要
  - `max_concurrent_agents <= roles.len()`
  - 各 role の `id`, `name`, `system_prompt`, `model` は必須
- `src-tauri/src/lib.rs` に migration v12 と新規 Tauri command を登録

## フロントエンド

- `src/types/index.ts` に `TeamRoleSetting`, `TeamConfiguration` を追加
- `src/components/ui/GlobalSettingsModal.tsx` に `チーム設定` タブを追加
- モーダル表示時に `get_team_configuration` を呼び、SQLite の内容を読み込むようにした
- 保存時に既存の `settings.json` 保存に加えて `save_team_configuration` を呼ぶようにした
- フロント側でも role 必須や並行数超過を検知し、保存前に警告を出すようにした
- `src/components/ui/TeamSettingsTab.tsx` を新規作成し、Team タブ部分を分離した
- Team タブでは以下を実装した
  - 最大並行稼働数のスライダー / 数値入力
  - ロールの追加 / 編集 / 削除
  - システムプロンプト編集
  - Anthropic API から取得したモデル一覧によるロールごとのモデル選択

## モデル一覧の扱い

- Team タブのモデル一覧は、既存の AI 設定タブと同じ `get_available_models('anthropic')` を利用する
- Anthropic API Key が設定されている場合のみ `モデル一覧を取得` を実行できる
- 取得済みの場合は select で選択し、未取得の場合は text input で手入力できる

## 実行コマンドに関する現状

- 現時点では、保存した role ごとの `model` は **まだ Claude CLI 実行コマンドには接続していない**
- 現在の `execute_claude_task` は以下の引数だけを使っている
  - `-p <prompt>`
  - `--permission-mode bypassPermissions`
  - `--add-dir <cwd>`
  - `--verbose`
- つまり `--model` はまだ未指定
- この接続は次の Epic で `claude_runner.rs` の引数設計とあわせて実施する想定

## 確認結果

- `cargo check` が通ることを確認
- `npm run build` が通ることを確認
- Team タブのモデル一覧取得 UI を追加した後も build が通ることを再確認

## 未実施の確認

- `npm run tauri dev` によるアプリ起動確認
- Team 設定の保存 → モーダル再オープンでの再読込確認
- バリデーションエラーの手動確認
- 既存の AI 設定 / プロジェクト設定タブの画面回帰確認

## 補足

- `npm run lint` は失敗するが、原因は今回の Team 設定変更ではなく既存の `src/components/project/ScaffoldingPanel.tsx` の lint error
- 今回の変更範囲では build / type check 上のエラーは発生していない
