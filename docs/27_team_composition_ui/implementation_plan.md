# Epic 27: AIチーム構成・ロール管理UIの構築 - 実装計画

## 背景と目的

次のEpicでは、複数の Dev エージェント（Claude Code CLI）を同時に起動し、タスクを並行実行できるようにする予定です。
本Epic 27 ではその前段として、**「何人まで同時に動かすか」** と **「各エージェントにどの役割・どのシステムプロンプト・どの Claude モデルを割り当てるか」** を管理できる設定基盤を整備します。

今回は **設定の保存と編集UIまで** を対象とし、実際の並行起動・スケジューリング・CLI 多重実行はスコープ外とします。

## スコープ決定事項

- チーム設定は **グローバル設定** として扱う
  - 理由: 要件上の配置候補が `GlobalSettingsModal` または専用 Team タブであり、現行の Dev Agent 実行もプロジェクト横断のアプリ設定と密接なため
- `roles` の各要素は **将来起動可能な Dev エージェント 1 枠** を表す
  - 同じ役割名を複数件登録することで「Frontend Dev を2人」のような表現を可能にする
- 既存の API キー / デフォルトAIプロバイダー設定はそのまま `settings.json` に残す
- 今回のモデル設定は **Dev チーム専用の Claude モデル文字列** として保持し、既存の `get_available_models` とは切り離す
  - 理由: 将来の Claude CLI 側の指定値と、Rig API の取得モデル一覧が必ずしも一致しないため

---

## 保存先の選定: SQLite を採用

### 比較

| 観点 | SQLite | Tauri Store |
|------|--------|-------------|
| 既存のドメインデータとの整合 | 強い | 弱い |
| 複数レコード（roles）の管理 | 得意 | 可能だが更新差分が粗い |
| マイグレーション | 可能 | 基本なし |
| 将来の並行実行との連携 | しやすい | 参照・整合性管理が弱い |
| 秘密情報の保存 | 向かない | 既存実装あり |

### 選定理由

現状のコードベースでは、以下の責務分担がすでに存在しています。

- **SQLite**: `projects`, `stories`, `tasks`, `team_chat_messages`, `task_dependencies` など、アプリの中核となる構造化データ
- **Store (`settings.json`)**: API キー、デフォルトAIプロバイダー、Inception Deck の一時的なUI状態など、軽量なローカル設定

今回のチーム構成は、

- ロールが複数件ある
- 並行実行数との整合が必要
- 将来 Epic で実行履歴や割当ロジックと結びつく
- マイグレーションで進化させる可能性が高い

という性質を持つため、**SQLite に入れる方が自然**です。

### 補足

- API キーや既存の AI Provider 設定は引き続き Store に残す
- Epic 27 では「Dev チーム構成だけを SQLite に追加」するハイブリッド構成にする

---

## データモデル設計

### マイグレーション案

**新規ファイル:** `src-tauri/migrations/12_team_configuration.sql`

```sql
CREATE TABLE IF NOT EXISTS team_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    max_concurrent_agents INTEGER NOT NULL DEFAULT 1
        CHECK (max_concurrent_agents BETWEEN 1 AND 5),
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO team_settings (id, max_concurrent_agents)
VALUES (1, 1);

CREATE TABLE IF NOT EXISTS team_roles (
    id TEXT PRIMARY KEY,
    team_settings_id INTEGER NOT NULL DEFAULT 1,
    name TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    model TEXT NOT NULL,
    sort_order INTEGER NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(team_settings_id) REFERENCES team_settings(id) ON DELETE CASCADE,
    UNIQUE(team_settings_id, sort_order)
);

INSERT OR IGNORE INTO team_roles (
    id, team_settings_id, name, system_prompt, model, sort_order
) VALUES (
    'seed-lead-engineer',
    1,
    'Lead Engineer',
    'あなたは優秀なリードエンジニアです。プロジェクト全体の技術方針を踏まえ、実装方針の整理、重要な設計判断、品質レビュー観点の提示を担当してください。',
    'claude-3-5-sonnet-20241022',
    0
);
```

### 設計意図

- `team_settings` はグローバル設定なので **singleton 行（id=1）** で十分
- `team_roles` は可変長リストのため別テーブル化
- `sort_order` で UI 上の表示順を保持
- `model` は TEXT で保持し、CLI 実行時にそのまま利用できるようにする
- 初回起動時の保存エラーを防ぐため、`Lead Engineer` を 1 件シード投入する

### Rust 構造体案

**対象:** `src-tauri/src/db.rs`

```rust
pub struct TeamSettings {
    pub max_concurrent_agents: i32,
}

pub struct TeamRole {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub model: String,
    pub sort_order: i32,
}

pub struct TeamConfiguration {
    pub max_concurrent_agents: i32,
    pub roles: Vec<TeamRole>,
}
```

### コマンド設計

**対象:** `src-tauri/src/db.rs`, `src-tauri/src/lib.rs`

#### 読み込み

```rust
#[tauri::command]
pub async fn get_team_configuration(app: AppHandle) -> Result<TeamConfiguration, String>
```

- `team_settings` と `team_roles` をまとめて返す
- ロールが未登録なら空配列を返す

#### 保存

```rust
#[tauri::command]
pub async fn save_team_configuration(
    app: AppHandle,
    config: TeamConfigurationInput
) -> Result<(), String>
```

- モーダル内では一覧をまとめて編集するため、**ロール単位 CRUD ではなく一括保存** を採用
- トランザクション内で以下を実行
  1. `team_settings` を UPSERT
  2. 既存 `team_roles` を削除
  3. 新しいロール配列を `sort_order` 順で再 INSERT

### バリデーション方針

- `max_concurrent_agents` は `1..=5`
- `roles` は 1 件以上必須
- `max_concurrent_agents <= roles.len()` を必須にする
- 各 role は `name`, `system_prompt`, `model` を必須
- 同一 role 名は許可する
  - 例: `Frontend Dev`, `Frontend Dev` の 2 枠
- 初期状態は `max_concurrent_agents = 1` かつ `Lead Engineer` 1件入りを期待値とする

---

## フロントエンド設計

### 採用UI: Global Settings に Team タブを追加

既存の `GlobalSettingsModal.tsx` が AI 設定とプロジェクト設定を持っているため、今回もここに **第3タブ「チーム設定」** を追加するのが最小変更です。

### 画面構成案

**対象候補:**

- `src/components/ui/GlobalSettingsModal.tsx`
- `src/components/ui/TeamSettingsTab.tsx`（新規切り出し推奨）

### UIモックアップ案

```text
┌ グローバル設定 ───────────────────────────────────────┐
│ [AI設定] [プロジェクト設定] [チーム設定]              │
├──────────────────────────────────────────────────────┤
│ 最大並行稼働数                                        │
│ [ 3 ]  ──●───────  (1〜5)                            │
│ 登録ロール数: 4 / 同時実行可能数: 3                  │
│                                                      │
│ Devチーム構成                                         │
│ ┌ Frontend Dev ─────────────── [model ▼] [削除] ┐   │
│ │ 役割名: [Frontend Dev____________________]     │   │
│ │ System Prompt                                  │   │
│ │ [UI実装と画面テストを担当する...___________]   │   │
│ └───────────────────────────────────────────────┘   │
│ ┌ Reviewer ──────────────────── [model ▼] [削除] ┐  │
│ │ 役割名: [Reviewer__________________________]    │  │
│ │ System Prompt                                  │  │
│ │ [差分レビューと品質確認を担当する..._______]   │  │
│ └───────────────────────────────────────────────┘  │
│                                                      │
│ [+ ロールを追加]                                     │
│                                   [キャンセル] [保存]│
└──────────────────────────────────────────────────────┘
```

### UI詳細

#### 1. 最大並行稼働数

- スライダー + 数値表示
- 最小 `1`、最大 `5`
- `roles.length` 未満までしか保存できない
- 保存不可のときはインライン警告を表示

#### 2. ロール一覧

各ロールカードに以下を表示:

- 役割名 `input`
- Claude モデル `select` または `input`
- システムプロンプト `textarea`
- 削除ボタン

#### 3. ロール追加

- `+ ロールを追加` ボタンで空カードを末尾追加
- 初期値は以下のプリセットどちらかを採用
  - 完全な空フォーム
  - 推奨テンプレート（Frontend Dev / Backend Dev / Reviewer）

今回は過剰な自動生成を避けるため、**空フォーム追加 + プレースホルダ提示** を第一案とする

#### 4. モデル入力UI

モデルは将来の CLI 実行時にそのまま使いたいため、以下のハイブリッドにする:

- プリセット選択肢
  - `claude-3-5-sonnet-20241022`
  - `claude-3-5-haiku-20241022`
- カスタム入力トグル

これにより、固定候補だけに閉じず、将来のモデル追加にも追従しやすくする

### フロントエンド状態管理

#### 新規型

**候補:** `src/types/index.ts`

```ts
export interface TeamRoleSetting {
  id: string;
  name: string;
  system_prompt: string;
  model: string;
  sort_order: number;
}

export interface TeamConfiguration {
  max_concurrent_agents: number;
  roles: TeamRoleSetting[];
}
```

#### データフロー

1. `GlobalSettingsModal` オープン時に `get_team_configuration` を呼ぶ
2. モーダル内でローカル draft state を保持する
3. 保存時に `save_team_configuration` を一括送信する
4. 成功時に toast 表示してモーダルを閉じる

### UI実装上の注意

- 既存の `GlobalSettingsModal.tsx` はすでに責務が重いため、Team タブ部分は新規コンポーネントへ切り出す
- `frontend-core` は参照対象に含めるが、今回のUI追加は主に `src/components/ui/**` 配下で完結させる
- `src/types/index.ts` を拡張する場合は、既存型との整合を崩さないように最小変更で行う

---

## 将来Epicへの接続ポイント

今回の保存構造は、次のマルチエージェントEpicで以下に利用できるようにする:

- `max_concurrent_agents` を実行上限として利用
- `roles` をエージェント生成時の入力として利用
- `model` を `claude` CLI のモデル指定オプションに接続
- `system_prompt` を各エージェント起動時のベースプロンプトに注入

### 今回はやらないこと

- Claude CLI の同時多重起動
- 実行中セッションの複数管理
- ロールごとの実行履歴保存
- ロールとタスクタイプの自動マッピング

---

## テスト方針

### DB / バックエンド

- migration `12_team_configuration.sql` が既存DBへ安全に適用できること
- `get_team_configuration` が初期状態で `max_concurrent_agents = 1` とシード済み `Lead Engineer` 1件を返すこと
- `save_team_configuration` がトランザクションで一括更新できること
- バリデーション違反
  - `max_concurrent_agents = 0`
  - `max_concurrent_agents = 6`
  - `roles = []`
  - `max_concurrent_agents > roles.len()`
  を拒否できること

### フロントエンド

- Team タブ表示時に既存の AI 設定 / プロジェクト設定タブが壊れないこと
- 並行稼働数の変更がUIに即時反映されること
- ロールの追加 / 編集 / 削除がローカル state に正しく反映されること
- 保存後に再度モーダルを開くと DB の内容が再表示されること
- 必須項目不足時に保存ボタンが無効化されること

### 回帰確認

- 既存の `settings.json` ベース設定
  - default provider
  - API keys
  - provider model
  がそのまま保存・読込できること
- `GlobalSettingsModal` の既存保存導線に影響しないこと

### 手動検証手順

1. アプリ起動後、グローバル設定を開く
2. `チーム設定` タブが表示されることを確認
3. `max_concurrent_agents = 3`、ロール3件以上を入力して保存
4. モーダルを閉じて再度開き、保存内容が維持されることを確認
5. ロールを1件削除し、`max_concurrent_agents` がロール数を超えた場合に警告されることを確認
6. 既存の AI 設定タブで API キー保存が従来通り動くことを確認

---

## 実装順の推奨

1. SQLite マイグレーション追加
2. `db.rs` に TeamConfiguration 用の構造体とコマンド追加
3. `lib.rs` にコマンド登録
4. Team タブ UI を追加
5. 保存・再読込の結線
6. バリデーションと手動検証

---

## 前提メモ

- 現時点の `execute_claude_task` は単一セッション前提で、モデル指定も受け取っていない
- そのため Epic 27 の責務は「将来使う設定の保存」までに留める
- 実行系の拡張は次Epicで `claude_runner.rs` 側の引数設計とあわせて実施する
