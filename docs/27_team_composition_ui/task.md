# Epic 27: タスク一覧

## Phase 1: データ保存基盤

- [x] 1-1. `src-tauri/migrations/12_team_configuration.sql` を作成
  - `team_settings` テーブルを追加
  - `team_roles` テーブルを追加
  - singleton 初期データを投入
  - `Lead Engineer` の初期ロールを 1 件シード投入

- [x] 1-2. `src-tauri/src/lib.rs` に Migration version 12 を登録

- [x] 1-3. `src-tauri/src/db.rs` に Team Configuration 用の構造体を追加
  - `TeamSettings`
  - `TeamRole`
  - `TeamConfiguration`
  - 保存用 input struct

- [x] 1-4. `src-tauri/src/db.rs` に新規コマンドを実装
  - `get_team_configuration`
  - `save_team_configuration`

- [x] 1-5. `save_team_configuration` にサーバー側バリデーションを実装
  - `max_concurrent_agents` が 1〜5
  - role 必須項目チェック
  - `max_concurrent_agents <= roles.len()`

- [x] 1-6. `src-tauri/src/lib.rs` の `generate_handler!` に新規コマンドを登録

## Phase 2: フロントエンド型と状態管理

- [x] 2-1. `src/types/index.ts` に Team 設定用の型を追加

- [x] 2-2. Team 設定の取得・保存を行うフロントエンド側の呼び出し処理を追加
  - `GlobalSettingsModal.tsx` 直書き、または小さな hook / helper に切り出し

- [x] 2-3. モーダル内の draft state を設計
  - 最大並行稼働数
  - role 一覧
  - バリデーションエラー

## Phase 3: Team タブ UI

- [x] 3-1. `GlobalSettingsModal.tsx` に `team` タブを追加

- [x] 3-2. Team タブ部分を新規コンポーネントへ分離
  - 候補: `src/components/ui/TeamSettingsTab.tsx`

- [x] 3-3. 最大並行稼働数の入力UIを追加
  - スライダー
  - 現在値表示
  - 保存不可条件の表示

- [x] 3-4. ロール一覧カード UI を追加
  - 役割名 input
  - システムプロンプト textarea
  - Claude モデル選択 / カスタム入力
  - 削除ボタン

- [x] 3-5. ロール追加導線を追加
  - 空フォーム追加
  - プレースホルダ文言

- [x] 3-6. 保存ボタンと既存モーダル保存導線を統合
  - Team タブを含めた保存
  - 成功時 toast

## Phase 4: 検証と回帰確認

- [ ] 4-1. DB マイグレーション適用確認

- [ ] 4-2. Team 設定の保存 → 再読込確認

- [ ] 4-3. 不正入力時のバリデーション確認
  - role 0 件
  - model 未入力
  - `max_concurrent_agents > roles.len()`

- [ ] 4-4. 既存の AI 設定 / プロジェクト設定タブの回帰確認

- [x] 4-5. 将来Epic向けの接続確認メモを残す
  - `claude_runner` にモデル指定を渡す拡張余地
  - 並列実行基盤との結合ポイント整理
