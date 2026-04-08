# Epic 35: AIアバターの実装とロール名変更 実装計画

## ステータス

- 状態: `Done`
- 実装開始条件: PO 承認済み
- 完了日: 2026-04-08
- 作成日: 2026-04-08

## Epic の目的

vicara の AI 体験を、単なる機能群ではなく「役割を持つチーム」として知覚できる状態へ進める。  
本 Epic では、既存の `AI Team Leader` を `POアシスタント` へ改名し、チャット画面およびカンバンボードで AI ごとの見た目を識別できるアバター表示を導入し、主要ドキュメントの役割定義を新ブランド方針へ揃える。

## スコープ

### ミッション 1: ロール名の全面変更
- UI 上の表示テキスト `AI Team Leader` を `POアシスタント` へ更新する。
- React コンポーネント内のローカル識別子は、破壊的影響が小さい範囲で `TeamLeader` から `PoAssistant` 系へ整理する。
- Rust バックエンドのシステムプロンプト上のペルソナ定義を `POアシスタント` として更新する。

### ミッション 2: アバター UI の実装
- 再利用可能なアバター表示コンポーネントを追加する。
- チャット画面では `POアシスタント` の発話に専用アバターを表示する。
- カンバンボードでは、担当ロールに応じて `POアシスタント` または `開発エージェント` のアバターを表示する。

### ミッション 3: ドキュメント更新
- `README.md`、`ARCHITECTURE.md`、`BACKLOG.md` に新ロール名と役割分担を反映する。
- vicara における AI の責務を、`実装担当 (Dev Agent)` と `意思決定支援担当 (POアシスタント)` に明確化する。

## 実装方針

### 1. 命名変更の方針
- ユーザーに見える文言はすべて `POアシスタント` に統一する。
- コンポーネント名やローカル変数名は、依存範囲が限定される箇所から `PoAssistant` へ置き換える。
- ただし、永続化やマイグレーション互換に影響する内部識別子は一段階で壊さない。
  - 例: `team_leader` の usage source kind、`8_ai_team_leader.sql`、Tauri command 名 `chat_with_team_leader`
- 上記の内部識別子は、Epic 35 の初回実装では「表示名変更とペルソナ変更を優先し、互換性を保つ」方針を推奨する。

### 2. アバターアセットの配置方針
- 推奨配置先: `public/avatars/`
- 推奨理由:
  - PO 側で後から画像を配置しても、フロントエンドの静的 import 不足でビルドを壊さない。
  - Tauri/Vite では `/avatars/...` の固定パス参照が扱いやすい。
  - 画像未配置時でも `img` の `onError` でアイコン fallback を出せる。

### 3. アセット命名案
- `public/avatars/dev-agent.png`
- `public/avatars/po-assistant.png`

### 4. 画像の割り当てルール
- `POアシスタント`: ネズミのフードを被った女性の画像を使用する。
- `開発エージェント`: 青い小さなロボットの画像を使用する。
- 既存の複数ロールは当面 `POアシスタント` かそれ以外かで分類し、それ以外はすべて `開発エージェント` の共通アバターを使う。
- 将来的にロールごとの個別アバターが必要になった場合は、`TeamRoleSetting` に `avatar_key` を追加する拡張余地を残す。

### 5. UI 実装の方針
- 新規コンポーネント候補:
  - `src/components/ai/Avatar.tsx`
  - `src/components/ai/avatarRegistry.ts`
- `Avatar.tsx` は以下を担う:
  - 画像表示
  - サイズ切り替え
  - 画像未配置時の fallback 表示
  - 角丸、枠線、背景グラデーションなどの見た目統一
- `avatarRegistry.ts` は以下を担う:
  - `po-assistant`
  - `dev-agent`
  - それぞれの表示名、画像パス、fallback アイコン情報

### 6. カンバン側のデータ参照方針
- 現在の `Task` は `assigned_role_id` のみを保持しており、`TaskCard` 単体では表示名やアバター種別を解決できない。
- そのため、`Board` で `get_team_configuration` を一度読み込み、`role.id -> role metadata` の map を生成して下位へ渡す構成を第一候補とする。
- 受け渡し経路:
  - `Board.tsx`
  - `StorySwimlane.tsx`
  - `StatusColumn.tsx`
  - `TaskCard.tsx`
- これにより、カードごとの個別 fetch を避け、描画負荷と責務分散を抑える。

## 対象ファイルと変更ポイント

| ファイル | 役割 | 変更内容 |
| --- | --- | --- |
| `src/components/ai/TeamLeaderSidebar.tsx` | PO相談サイドバー | コンポーネント名・表示文言を `POアシスタント` に変更。ヘッダー、空状態、メッセージ気泡、ローディング行にアバター表示を追加。 |
| `src/App.tsx` | 3ペイン UI の親 | `TeamLeaderSidebar` の import / 使用箇所 / ボタン文言を `POアシスタント` 基準へ更新。 |
| `src/components/ai/Avatar.tsx` | 新規 | 共通アバター描画コンポーネントを追加。 |
| `src/components/ai/avatarRegistry.ts` | 新規 | `POアシスタント` と `開発エージェント` の画像パス・fallback 定義を集約。 |
| `src/components/kanban/Board.tsx` | スプリントボード親 | チーム設定読み込みと role map 構築。下位コンポーネントへ role metadata を渡す。 |
| `src/components/kanban/StorySwimlane.tsx` | ストーリー行 | role metadata の props relay。必要に応じてストーリー内追加タスク導線との整合を取る。 |
| `src/components/kanban/StatusColumn.tsx` | ステータス列 | `TaskCard` へ role metadata を渡す。 |
| `src/components/kanban/TaskCard.tsx` | タスクカード | 担当ロール名とアバターをカード上に表示。`POアシスタント` / `開発エージェント` の識別を UI 上で可視化。 |
| `src/components/board/TaskFormModal.tsx` | タスク編集モーダル | 担当ロール選択 UI にアバター表現を入れるかを調整。少なくとも新名称との整合を確認。 |
| `src/components/ui/GlobalSettingsModal.tsx` | usage 表示 | `team_leader: 'Leader'` を `PO` または `PO Assistant` 系表示へ更新。 |
| `src-tauri/src/ai.rs` | PO相談バックエンド | システムプロンプトのペルソナ文言を `POアシスタント` へ変更。必要に応じてローカル struct 名も整理。 |
| `src-tauri/src/db.rs` | コメント/説明 | `AI Team Leader` 表記コメントを `POアシスタント` に追随。 |
| `src-tauri/src/lib.rs` | Tauri command/export | command 名の全面 rename を行うか、互換性優先で据え置くかの判断対象。 |
| `src-tauri/src/rig_provider.rs` | AI呼び出しラッパ | command / helper 名の rename 対象候補。初回は据え置き推奨。 |
| `README.md` | 主要紹介文書 | `AI Team Leader` を `POアシスタント` へ更新し、`開発エージェント` との役割分担を追記。アバター導入後の見え方も反映。 |
| `ARCHITECTURE.md` | 設計書 | vicara における AI 二層構造を明記。意思決定支援と実装実行の責務境界を追記。 |
| `BACKLOG.md` | 主要運用文書 | 用語統一と、将来的なアバター拡張や role metadata 一般化の課題を必要最小限で追記。 |

## 変更対象の補足

### frontend-core 取り扱い
- `src/App.tsx`、`src/components/ui/GlobalSettingsModal.tsx`、`src/types/**`、`src/context/**`、`src/hooks/**` は `frontend-core` に属する。
- リポジトリ運用ルール上、`frontend-core` は参照必須かつ慎重変更対象である。
- Epic 35 では `App.tsx` と `GlobalSettingsModal.tsx` は変更候補に含むが、型定義や Context/Hooks の構造変更は初回実装では避ける。

### 型定義の扱い
- `TeamRoleSetting` に新しい永続フィールドを追加する案は、拡張性は高いが Rust/フロント双方の型変更が必要になる。
- 今回は安定性優先のため、初回は `role.name === 'POアシスタント'` 判定を起点にし、将来の `avatar_key` 追加は別タスク化する方針を推奨する。

## リスクと判断ポイント

### 1. 内部識別子の rename 範囲
- `chat_with_team_leader`、`team_leader`、`ai_team_leader` を同時に rename すると、使用量集計や既存 migration 参照まで連鎖する。
- 初回実装では UI/文言/ペルソナを優先し、内部 ID は据え置く方が安全。

### 2. 画像未配置時の UX
- `public/avatars/` を採用し、未配置時は fallback を表示すれば、PO の画像投入タイミングと独立して実装を進められる。

### 3. カンバンのデータ流し込み
- `TaskCard` 単独 fetch は重複通信になりやすい。
- `Board` で一括解決して props relay する構成のほうが、今回の規模では見通しがよい。

## テスト方針

### 1. 静的確認
- `AI Team Leader` / `TeamLeader` 文字列の残存検索を行い、意図的に残す内部識別子を除いて整理できていることを確認する。
- `POアシスタント` 表示が README、主要 UI、Rust システムプロンプトで揃っていることを確認する。

### 2. フロントエンド確認
- `npm run build` を実行し、React 側の型・import・props 変更でビルドエラーが出ないことを確認する。
- サイドバー開閉、空状態、チャット中、ローディング中の各状態でアバター崩れがないことを目視確認する。
- カンバンボードで、担当ロールあり/なしのタスクカード表示が破綻しないことを確認する。

### 3. バックエンド確認
- `cargo test --manifest-path src-tauri/Cargo.toml` を実行し、Rust 側の回帰がないことを確認する。
- `chat_with_team_leader` の呼び出し経路が rename の影響で壊れていないことを確認する。

### 4. 手動受け入れ確認
- `POアシスタント` のサイドバーを開いたときに新名称と新アバターで表示される。
- カンバン上の開発担当タスクが `開発エージェント` のアバターで識別できる。
- README/ARCHITECTURE/BACKLOG を読んだとき、vicara における `意思決定支援担当` と `実装担当` の違いが伝わる。

## 実装結果

- 内部識別子 `team_leader` / `chat_with_team_leader` / migration 名は据え置き、UI とペルソナのみ `POアシスタント` へ更新した。
- アバターアセットは `public/avatars/po-assistant.png` と `public/avatars/dev-agent.png` を前提とする実装へ切り替えた。
- 画像未配置時でも `Avatar.tsx` が fallback アイコンを描画するため、画面はクラッシュしない。
- カンバン上のアバター判定は初回方針どおり「ロール名が `POアシスタント` か、それ以外か」で解決した。

## テスト実施結果

- `npm run build`: 成功
- `cargo test --manifest-path src-tauri/Cargo.toml`: 成功

