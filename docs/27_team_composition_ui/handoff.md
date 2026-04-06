# Epic 27: 引き継ぎ書

## 現在の状態

- Phase 1〜3 の実装は完了
- Team 構成の保存基盤と設定 UI は追加済み
- 自動検証は `cargo check` と `npm run build` まで完了
- 手動確認は未着手

## 重要な仕様

- Team 設定は SQLite に保存している
- `team_settings` は singleton 行 `id = 1`
- `team_roles` は role の一覧を保持する
- 初回起動時の UX を崩さないため、`Lead Engineer` を 1 件シード投入している
- 保存時の制約は `max_concurrent_agents <= roles.len()`

## モデル一覧について

- Team タブの Claude モデル一覧は `get_available_models('anthropic')` を使って取得する
- つまり、既存の AI 設定タブと同じ Anthropic API ベースの一覧取得を再利用している
- Anthropic API Key 未設定時は一覧取得ボタンを無効化している

## 実行コマンドの現状

- 重要: role ごとの `model` は **まだ Claude CLI 実行には使っていない**
- 現在の `execute_claude_task` の実行引数は以下

```text
claude -p <prompt> --permission-mode bypassPermissions --add-dir <cwd> --verbose
```

- `--model` は未指定
- そのため、Epic 27 は「モデル設定を保存できる状態」までで止めている
- 次の Epic で `claude_runner.rs` と呼び出し元に `model` を渡す設計が必要

## 次にやるべきこと

- `npm run tauri dev` でアプリを起動し、Team タブを手動確認する
- 以下を順に確認する
  - 初回表示で `Lead Engineer` が入っていること
  - `max_concurrent_agents` の変更が保存できること
  - role の追加 / 編集 / 削除が保存できること
  - モデル一覧取得後、各 role の select で Claude モデルを選べること
  - モーダルを閉じて再度開いたときに内容が再読込されること
  - AI 設定タブ / プロジェクト設定タブに回帰がないこと

## 既知の注意点

- `npm run lint` は既存の `src/components/project/ScaffoldingPanel.tsx` の lint error で失敗する
- これは今回の変更ではない
- Team 設定まわりは build までは通っている

## 主な関連ファイル

- `src-tauri/migrations/12_team_configuration.sql`
- `src-tauri/src/db.rs`
- `src-tauri/src/lib.rs`
- `src/components/ui/GlobalSettingsModal.tsx`
- `src/components/ui/TeamSettingsTab.tsx`
- `src/types/index.ts`
- `docs/27_team_composition_ui/task.md`
- `docs/27_team_composition_ui/walkthrough.md`
- `docs/27_team_composition_ui/handoff.md`

## 推奨確認コマンド

- `cargo check`
- `npm run build`
- `npm run tauri dev`

## task.md の見方

- 実装タスクは Phase 1〜3 を完了済みとして更新済み
- Phase 4 は主に手動確認タスクとして残している
- 引き継ぎメモ作成に相当する `4-5` は本ファイル作成により完了扱いにしてよい
