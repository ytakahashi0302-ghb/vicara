# Epic 35: AIアバターの実装とロール名変更 タスクリスト

## ステータス

- 状態: `Done`
- 着手条件: PO 承認済み
- 完了日: 2026-04-08
- 作成日: 2026-04-08

## 実行順序

### 1. 命名変更方針の確定
- [x] `AI Team Leader` / `TeamLeader` の残存箇所を最終確認する。
- [x] UI 文言、ローカル識別子、内部識別子のどこまでを Epic 35 の対象に含めるか確定する。
- [x] `team_leader` 系の内部 ID は互換性優先で据え置く方針で実装する。

### 2. アバター基盤の追加
- [x] `public/avatars/` を前提としたアセット配置ルールを確定する。
- [x] `Avatar.tsx` を追加し、画像・fallback・サイズ切り替えを共通化する。
- [x] `avatarRegistry.ts` を追加し、`POアシスタント` / `開発エージェント` の見た目定義を集約する。

### 3. POアシスタント UI への改修
- [x] `TeamLeaderSidebar.tsx` を `PoAssistantSidebar.tsx` へ整理し、`POアシスタント` 表示へ更新する。
- [x] サイドバーのヘッダー、空状態、メッセージ行、ローディング行にアバターを導入する。
- [x] `App.tsx` のサイドバートグル文言と import 名を新名称へ揃える。

### 4. カンバンへのアバター表示導入
- [x] `Board.tsx` で `get_team_configuration` を読み込み、`role.id -> metadata` を解決する。
- [x] `StorySwimlane.tsx` と `StatusColumn.tsx` で role metadata を relay する。
- [x] `TaskCard.tsx` に担当ロール名とアバター表示を追加する。
- [x] `TaskFormModal.tsx` は既存 UI のまま維持し、ロール名表示との整合のみ確認する。

### 5. バックエンドと文言整合
- [x] `src-tauri/src/ai.rs` のシステムプロンプトを `POアシスタント` ペルソナへ更新する。
- [x] `src-tauri/src/db.rs` のコメントや説明表記を更新する。
- [x] `GlobalSettingsModal.tsx` の usage ラベル `Leader` を新名称系に合わせる。

### 6. ドキュメント更新
- [x] `README.md` に `POアシスタント` と `開発エージェント` の役割分担を追記する。
- [x] `ARCHITECTURE.md` に意思決定支援担当と実装担当の責務境界を追記する。
- [x] `BACKLOG.md` に用語統一と将来のアバター拡張観点を整理する。

### 7. 検証
- [x] 残存文字列検索で `AI Team Leader` の表層文言が主要 UI / 主要ドキュメントから解消されていることを確認する。
- [x] `npm run build` を実行する。
- [x] `cargo test --manifest-path src-tauri/Cargo.toml` を実行する。
- [ ] チャット画面とカンバン画面でアバター表示を手動確認する。
- [ ] 画像未配置時の fallback 表示を手動確認する。

## 完了条件

- [x] ユーザー向け表記が `POアシスタント` に統一されている。
- [x] `POアシスタント` と `開発エージェント` が UI 上で視覚的に識別できる実装になっている。
- [x] README / ARCHITECTURE / BACKLOG に新しい役割分担が反映されている。
- [x] フロントエンドと Rust バックエンドのビルド・テストが通る。

## 補足

- 画像ファイルの実配置は PO 側作業を前提としており、現時点では fallback 表示で安全に動作する設計になっている。
