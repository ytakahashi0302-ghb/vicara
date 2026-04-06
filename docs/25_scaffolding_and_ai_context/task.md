# Epic 25: タスク一覧

## Phase 1: Backend基盤（scaffolding.rs）
- [ ] 型定義（TechStackInfo, ScaffoldStrategy, ScaffoldStatus）
- [ ] `detect_tech_stack()` — ARCHITECTURE.md解析による技術スタック検出
- [ ] `check_scaffold_status()` — 既存Scaffold有無チェック
- [ ] `generate_agent_md()` — 参照ポインタ方式AGENT.md生成
- [ ] `generate_claude_settings()` — .claude/settings.json生成
- [ ] `lib.rs` にmod宣言・コマンド登録

## Phase 2: Scaffold実行コマンド
- [ ] `execute_scaffold_cli()` — PtyManager経由でCLIスキャフォールド実行（ストリーミング出力対応）
- [ ] `execute_scaffold_ai()` — Claude CLI経由でAI生成スキャフォールド実行
- [ ] Scaffolding用Tauriイベント定義（`scaffold_output`, `scaffold_exit`）

## Phase 3: Frontend — ScaffoldingPanel
- [ ] `ScaffoldingPanel.tsx` 作成 — 技術スタック検出結果表示・実行ボタン・状態管理
- [ ] Scaffold出力のTerminalDockストリーミング連携
- [ ] AGENT.md完了プレビュー表示

## Phase 4: 既存UIとの統合
- [ ] `InceptionDeck.tsx` — Phase 5完了後にScaffoldingPanel表示
- [ ] `ProjectSettings.tsx` — 手動Scaffoldingトリガーボタン追加

## Phase 5: 検証・ドキュメント
- [ ] React/Viteプロジェクトでのe2eテスト（CliScaffold戦略）
- [ ] バニラプロジェクトでのAI生成テスト（AiGenerated戦略）
- [ ] AGENT.md + .claude/settings.json 生成内容の正確性確認
- [ ] `walkthrough.md` 作成
