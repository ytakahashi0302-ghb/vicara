# Epic 35: AIアバターの実装とロール名変更 修正内容の確認

## ステータス

- 状態: `Done`
- 更新条件: 実装完了済み
- 完了日: 2026-04-08
- 作成日: 2026-04-08

## 実装内容

- `src/components/ai/Avatar.tsx` と `src/components/ai/avatarRegistry.ts` を追加し、`public/avatars/` 前提の共通アバター基盤を実装した。
- `src/components/ai/TeamLeaderSidebar.tsx` を `src/components/ai/PoAssistantSidebar.tsx` へ整理し、サイドバー全体の表示名を `POアシスタント` に変更した。
- `src/App.tsx` を更新し、ヘッダーのトグルから `POアシスタント` を開閉できるようにした。
- `src/components/kanban/Board.tsx` から `TaskCard.tsx` までの props 経路を拡張し、担当ロール名に応じたアバター表示を追加した。
- `src-tauri/src/ai.rs` のシステムプロンプトを `POアシスタント` ペルソナへ変更し、内部 command 名や usage key は据え置いた。
- `README.md`、`ARCHITECTURE.md`、`BACKLOG.md` を更新し、`POアシスタント` と `開発エージェント` の責務分担を明記した。

## テスト結果

- `npm run build`: 成功
- `cargo test --manifest-path src-tauri/Cargo.toml`: 成功

## 補足

- アバター画像が未配置でも、fallback アイコン表示により画面が崩れないようにしている。
- 画像そのものの見え方は、PO 側で `public/avatars/` にファイル配置後に手動確認する前提。
