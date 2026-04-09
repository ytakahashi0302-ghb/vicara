# Epic 43: トランスポート層統一 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 42 完了
- 作成日: 2026-04-09

## Epic の目的

Phase 1〜3-A で段階的に追加してきた各種 AI ツール対応を、一貫性のある統一アーキテクチャとして完成させる。設定の分散を解消し、ユーザーが直感的に「PO は Ollama、Lead Dev は Claude Code、Frontend Dev は Gemini CLI」といったチーム編成を行える最終形を実現する。

## スコープ

### 対象ファイル（変更）
- `src-tauri/src/db.rs` — team_roles スキーマ拡張（transport カラム追加の可能性）
- `src-tauri/src/ai.rs` — PO transport 解決の統一
- `src-tauri/src/rig_provider.rs` — 設定解決の統一
- `src/components/ui/TeamSettingsTab.tsx` — transport 選択 UI
- `src/components/ui/SetupStatusTab.tsx` — 統合ビュー
- `src/components/ui/GlobalSettingsModal.tsx` — 設定構造の整理

### 対象ファイル（新規マイグレーション）
- `src-tauri/migrations/XX_transport_unification.sql` — 必要に応じて

### 対象外
- 各 CLI Runner の実装変更（Epic 37-38 で完成済み）
- 各 API Provider の実装変更（Epic 41 で完成済み）

## 実装方針

### 1. 統一設定モデル

Phase 1〜3-A を経て、設定が以下のように分散している（想定）:

```
settings.json:
  default-ai-provider        → PO の API プロバイダー
  anthropic-api-key           → Anthropic API キー
  gemini-api-key              → Gemini API キー
  openai-api-key              → OpenAI API キー（Epic 41）
  ollama-endpoint             → Ollama エンドポイント（Epic 41）
  po-assistant-transport      → PO の transport 種別（Epic 42）
  po-assistant-cli-type       → PO の CLI 種別（Epic 42）
  po-assistant-cli-model      → PO の CLI モデル（Epic 42）

team_roles テーブル:
  cli_type                    → Dev エージェントの CLI 種別（Epic 37）
  model                       → Dev エージェントのモデル
```

**統一後のモデル:**

```
team_roles テーブル:
  transport    TEXT NOT NULL DEFAULT 'cli'    -- 'api' | 'cli' | 'local'
  provider     TEXT NOT NULL DEFAULT 'claude' -- 具体的なツール名
  model        TEXT NOT NULL                  -- モデル名

settings.json:
  api-keys:     { anthropic, gemini, openai }     -- 認証情報のみ
  endpoints:    { ollama }                         -- エンドポイント設定のみ
  po-transport: transport + provider + model       -- PO 固有設定
```

### 2. provider の命名規則

transport と provider の組み合わせ:

| transport | provider 値 | 説明 |
|-----------|------------|------|
| `cli` | `claude` | Claude Code CLI |
| `cli` | `gemini` | Gemini CLI |
| `cli` | `codex` | Codex CLI |
| `api` | `anthropic` | Anthropic API |
| `api` | `gemini` | Gemini API |
| `api` | `openai` | OpenAI API |
| `local` | `ollama` | Ollama ローカル LLM |

### 3. DB マイグレーション（必要に応じて）

Epic 37 で追加した `cli_type` を `provider` にリネームし、`transport` カラムを追加する:

```sql
-- transport カラムを追加（既存データは全て cli）
ALTER TABLE team_roles ADD COLUMN transport TEXT NOT NULL DEFAULT 'cli';
-- cli_type を provider にリネーム（SQLite は ALTER RENAME COLUMN 対応）
ALTER TABLE team_roles RENAME COLUMN cli_type TO provider;
```

**注意:** SQLite の ALTER RENAME COLUMN は 3.25.0+ で対応。Tauri バンドルの SQLite バージョンを確認すること。非対応の場合はテーブル再作成方式で対応。

### 4. チーム設定 UI の最終形

```
┌──────────────────────────────────────┐
│ [アバター] Lead Engineer              │
│                                      │
│ 実行方式:  [CLI ▾]                    │
│ ツール:    [Claude Code ▾]           │
│ モデル:    [claude-sonnet-4 ▾]       │
│                                      │
│ システムプロンプト: [テキスト]          │
└──────────────────────────────────────┘

┌──────────────────────────────────────┐
│ [アバター] Frontend Developer         │
│                                      │
│ 実行方式:  [Local ▾]                  │
│ ツール:    [Ollama ▾]                │
│ モデル:    [qwen2.5-coder:14b ▾]     │
│                                      │
│ システムプロンプト: [テキスト]          │
└──────────────────────────────────────┘
```

transport 変更時の連鎖:
```
transport: CLI  → provider: claude/gemini/codex  → model: CLI 別デフォルト
transport: API  → provider: anthropic/gemini/openai → model: API 別デフォルト
transport: Local → provider: ollama               → model: Ollama モデル一覧
```

### 5. セットアップ状況タブの推奨構成

ユーザーの環境に基づいた推奨表示の例:

```
✅ 推奨構成（あなたの環境に基づく）:
  PO アシスタント: Anthropic API (claude-haiku-4-5) — 高速・低コスト
  Dev エージェント: Claude Code CLI (claude-sonnet-4) — 高品質コード生成

⚡ 代替構成:
  完全無料構成: Ollama (PO) + Gemini CLI (Dev) — Gemini CLI は無料枠あり
```

### 6. Observability の transport 別集計

LLM Usage ダッシュボードに追加:

```
┌──────────────────────────────────────┐
│ Transport 別コスト                    │
│  API:   $1.23 (45 requests)         │
│  CLI:   N/A   (120 requests)        │
│  Local: Free  (30 requests)         │
└──────────────────────────────────────┘
```

## テスト方針

- 異種 transport 混合チームでの一連のフロー:
  1. PO アシスタント (Ollama) でアイデア → ストーリー作成
  2. タスク生成
  3. Dev エージェント (Claude Code CLI) でタスク実行
  4. 別ロール (Gemini CLI) で別タスク実行
- 設定変更の永続化と即時反映
- LLM Usage の transport / provider / model 記録の正確性
- マイグレーション適用後の既存データ互換性
- 全 transport / provider 組み合わせでの基本動作確認
