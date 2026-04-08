# vicara 開発ガイドライン (Rule.md)

> **【AIへの絶対指示】** 本ファイルは実装ルール（How）を定義する。いかなる提案・実装においても、以下の規約を厳守せよ。

## 1. 技術スタック
- **Frontend**: React (Hooks中心), TypeScript (厳格な型定義/`any`禁止), Tailwind CSS v4
- **UI Library**: dnd-kit (カンバン), lucide-react (アイコン)
- **Backend/Desktop**: Tauri v2 (Rust)
- **Database**: SQLite (ローカルファイル), `tauri-plugin-sql` / `@tauri-apps/plugin-sql`

## 2. コーディング規約とアーキテクチャ
- **UIのリアクティビティ (最重要)**: **Mutation後にUIが即時更新されない実装は「不正」とする。** 原則としてState更新で即時反映（Optimistic Update）し、必要に応じてRefetchで整合性を担保すること。
- **状態管理戦略**: 原則はContext/State。複数コンポーネント間でStateが頻繁に共有される、または更新ロジックが分岐し始めた場合に限り導入を提案せよ。その際、状態管理ライブラリは軽量なもの（例: Zustand）を優先し、過剰な抽象化（例: Redux等）を避けること。
- **SQLの安全性**: **文字列連結によるSQL生成は厳禁。** 必ずプレースホルダーを用いてインジェクション対策を徹底せよ。フロントエンドでの複雑なSQL組み立ては避け、Rust側での処理を優先せよ。
- **UIの言語**: 内部ロジックは英語でよいが、エンドユーザーが触れるUI（ボタン、通知等）は常に自然な日本語で実装せよ。

## 3. エージェントの振る舞い（運用ルール）
- **実装前のプロセス**: コーディング着手前に、(1)アプローチ (2)変更対象ファイル (3)影響範囲 (4)リスク（破壊的変更・パフォーマンス・UX劣化・将来拡張性への影響）を明示し、POの承認を得ること。
- **破壊的変更の定義**: 既存のロジック変更・削除・データ構造変更・API仕様変更を伴う場合は、必ず事前にPOの許可を得ること。
- **完了報告とテスト**: 手動検証を依頼する際は、必ず `walkthrough.md` を出力し「変更内容・テスト手順・検証結果」をエビデンスとして提示せよ。
- **セッションの引き継ぎ**: チャット終了時は、必ず現在の進捗と次やるべきタスクをまとめた `handoff.md` (Layer 2) を作成・更新せよ。
- **ドキュメントの同期**: 設計・仕様・挙動に影響する変更を行った場合は、以下のファイルを必ず自己更新せよ。
  - `PRODUCT_CONTEXT.md` (Why: 設計方針・思想)
  - `Rule.md` (How: このファイル)
  - `ARCHITECTURE.md` (What: 詳細設計・DB/API仕様)

## 4. 技術的な特記事項 (Tips & Constraints)
- **Tailwind v4**: `@tailwindcss/postcss` を使用し、CSSでは `@import "tailwindcss";` を用いること。
- **Tauri v2 Permissions**: `capabilities/default.json` に `"sql:default"`, `"sql:allow-execute"`, `"sql:allow-select"` を含めること。

> ※注記: バグや技術的負債（Tech Debt）は本ファイルに追記せず、 `BACKLOG.md` に集約せよ。

