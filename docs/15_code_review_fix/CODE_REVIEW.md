# MicroScrum AI コードレビュー

> レビュー実施日：2026-03-09

---

## 全体評価

**良い点**
- Tauri/Rust + React + SQLite の構成は適切。ローカルファースト設計と一致している
- DB操作はパラメータバインドで SQLインジェクション対策済み
- マイグレーションがバージョン管理されており、既存データ保全の考慮がある（`INSERT … SELECT` + `DROP` + `RENAME` パターン）
- React Context の階層（WorkspaceProvider → SprintTimerProvider → ScrumProvider）が依存関係と合致している
- `useCallback` によってフック関数の参照安定性が確保されている
- ドキュメント（PRODUCT_CONTEXT, ARCHITECTURE, FUTURE_CONCEPT）が充実している

---

## 問題点・改善点

### 🔴 Critical（バグ・データ損失リスク）

#### 1. `archive_sprint` がトランザクションなし (`src-tauri/src/db.rs:317-358`)
スプリントINSERT → タスクUPDATE → ストーリーUPDATE の3ステップが別々のクエリ。
途中で失敗すると「スプリントレコードは存在するがタスク/ストーリーが未アーカイブ」の不整合状態になる。
→ **対策**: `BEGIN TRANSACTION … COMMIT / ROLLBACK` でラップする

#### 2. SQLite の外部キー制約が無効のまま (`src-tauri/src/db.rs`, `src-tauri/src/lib.rs`)
SQLite はデフォルトで `PRAGMA foreign_keys = OFF`。
`ON DELETE CASCADE` や `ON DELETE SET NULL` が定義されていても機能しない。
`delete_project` を実行しても子レコード（stories/tasks/sprints）が残る。
→ **対策**: DB接続後に `PRAGMA foreign_keys = ON` を実行する

#### 3. ストーリーにタスクが0件の場合の誤アーカイブ (`src-tauri/src/db.rs:341-355`)
```sql
NOT EXISTS (SELECT 1 FROM tasks WHERE tasks.story_id = stories.id AND tasks.sprint_id IS NULL)
```
タスクが1件もないストーリーは条件が真になるため、スプリント完了時に自動アーカイブされる。
ユーザーが意図しないアーカイブが発生しうる。

---

### 🟡 Medium（品質・保守性）

#### 4. APIキー取得ロジックの重複 (`src-tauri/src/ai.rs`)
`generate_tasks_from_story` と `refine_idea` の両方に同一のキー取得コードが約20行ずつ重複している。
→ ヘルパー関数 `get_api_key(store, provider) -> Result<String, String>` に切り出す

#### 5. JSONパース正規表現がネストに対応できない可能性 (`src-tauri/src/ai.rs:159`)
```rust
let re = regex::Regex::new(r"(?s)\[.*?\]")
```
`[{"key": [1,2]}]` のようなネストされたJSON配列に対して最短マッチで途中で切れる可能性がある。
→ Gemini は `responseMimeType: "application/json"` を使えば raw JSONが返るため、`generate_tasks_from_story` でも同様に structured output を利用する

#### 6. `WorkspaceContext` の初期値がハードコード (`src/context/WorkspaceContext.tsx:18`)
```typescript
const [currentProjectId, setCurrentProjectIdState] = useState<string>('default');
```
`'default'` プロジェクトが削除された場合、データ取得が空になるが視覚的なエラーは出ない。
プロジェクト一覧取得後に先頭の有効なプロジェクトへ切り替えるロジックが必要。

#### 7. AIモデルのバージョンがハードコード (`src-tauri/src/ai.rs:122, 132`)
```rust
"model": "claude-3-5-sonnet-20241022"
```
設定画面から変更できないため、新モデルへの切り替えにコード変更が必要。

---

### 🟢 Minor（小規模改善）

#### 8. デバッグログが残っている (`src-tauri/src/db.rs:174, 228`)
```rust
println!("Fetched stories: {:?}", stories);
println!("Fetched tasks: {:?}", tasks);
```
プロダクションビルドでもログが出力される。リリース前に削除する。

#### 9. `greet` コマンドが残っている (`src-tauri/src/lib.rs:7-9`)
Tauri テンプレートのスキャフォールディングコード。未使用。

#### 10. `update_project` / `delete_project` が WorkspaceContext に未公開
Tauri コマンドは存在するが、ContextのAPIには含まれていない。
コンポーネントが `invoke` を直接呼ぶか、Context経由で呼べるか不明確。

#### 11. 変異後の全件再フェッチ (`src/hooks/useStories.ts`, `src/hooks/useTasks.ts`)
`addStory` → `fetchStories()` パターン。今は問題ないが、データが増えると遅延/ちらつきの原因になりうる。
楽観的UIは既にカンバン移動に実装されているため、CRUD操作も同様のパターンに揃えると一貫性が増す。

---

## アーキテクチャ上の懸念

### `sprint_id IS NULL` = アクティブ の意味論的曖昧さ
現状、stories/tasks の「アクティブ」を `sprint_id IS NULL` で判定している。
これは「スプリント未割当」と「アーカイブ済み」を区別できない設計。
将来のスプリントプランニング（ストーリーをスプリントに事前割当する機能）を追加する際に破綻する可能性がある。

**将来案**: `archived BOOLEAN DEFAULT FALSE` カラムを追加し、アーカイブ状態を明示的に管理する。

---

## 設計思想へのコメント

### ✅ スクラムを共通プロトコルとする発想は強い
「AIが次に何をするかスクラムを知っていれば分かる → 安心して任せられる」という論理は説得力がある。
ただし、**現状の実装はスクラムの「形式」（カンバン・スプリントタイマー）のみで、「プロトコル」としての機能（AI同士の状態遷移の明確化）はまだ存在しない**。
将来のエージェント実装でこのギャップを埋めることが設計の核心になる。

### ✅ スプリントを承認スコープの単位にする発想は秀逸
「毎回確認」でも「全自動」でもなく、スプリント単位で一括承認するアイディアは、
Claude Codeのhooksやpermission modelと相性が良い。実装難度は高いが方向性は正しい。

### ⚠️ `sprint_id` によるアーカイブ管理はスプリントプランニングと相容れない
現在「`sprint_id IS NULL` = バックログ（アクティブ）」という設計になっているが、
将来のスプリントプランニング機能（ストーリーをスプリントに事前割り当て）を追加すると、
割り当て済みだがまだ実行中のストーリーが「アーカイブ済み」と区別できなくなる。
**これは将来的な設計の負債として早めに対処すべき。**

### ⚠️ コスト管理の「予算（¥）」は現状UI・DBともに未実装
FUTURE_CONCEPT.md でコスト管理の設計が詳細に描かれているが、
現状の `sprints` テーブルには `budget`・`cost_actual` カラムがなく、
AI呼び出し時のトークン消費量も記録していない。
将来実装する際は `src-tauri/src/ai.rs` の各呼び出しに usage 計測を追加する必要がある。

### ✅ SMを「節目にのみ登場するフラットな視点」として設計した判断
コンテキストに染まったエージェントは方向性を疑えないという観察は正しい。
ただし、「SMは情報を絞る」という設計は、実装時にプロンプト管理の複雑さを増す。
**SMが使う情報（1〜2行のプロダクト概要 + スプリントゴール + チェック観点）を
DBテーブルとして明示的に管理する設計**にしておくと、将来の実装が楽になる。

### ✅ 「仕様は常に不完全」というスタンスと循環型の設計
仕様駆動開発との差異として「仕様が育っていく」と明記されている点は良い。
PRODUCT_CONTEXT.md を「AIへの引き継ぎ専用」として位置づけ、
スプリントのたびに更新する運用設計は、現状でも機能しており評価できる。

---

## 将来機能へのコメント（FUTURE_CONCEPT.md）

### エージェント学習ループ（agent_retrospectives）
設計方針は明確で実装可能。いくつか検討点：

1. **直近Nスプリントの「N」の決め方**：無限に蓄積すると古い情報がノイズになる。
   スプリントごとの重み付け（直近を重視）か、「直近3〜5スプリント」を固定するのが現実的。

2. **`next_action` の品質問題**：AIが生成した改善提案がそのまま次スプリントのコンテキストに注入されると、
   質の低い提案が蓄積してノイズになりうる。**人間が承認・編集できる確認UIを挟む**方が安全。

3. **agent_retrospectivesの`sprint_id`型**：FUTURE_CONCEPTでは `INTEGER` だが、
   現状の `sprints.id` は `TEXT (UUID)` 。型を合わせる必要がある。

### 操作ログとロールバック（agent_operations）
スプリント単位でのロールバックはコンセプトとして面白いが、実装難度が高い：

- ファイルの `snapshot`（変更前の内容）をSQLiteのTEXTカラムに格納すると、
  大きなファイルの場合にDBサイズが爆発する可能性がある。
- **Gitを使ったロールバック**（スプリント開始時にコミット、ロールバック時に `git reset`）の方が
  シンプルで信頼性が高い。TauriでGitコマンドを呼ぶ設計も検討に値する。

### コスト管理の2軸設計（時間 × 予算）
UI設計（進捗バー2本表示）は直感的でわかりやすい。実装上の注意点：

- APIの usage（input/output tokens）はAnthropicもGeminiも返却してくれるが、
  **モデルごとの単価を設定として保持する**必要がある（単価は変わることもある）。
- 円換算はリアルタイムレートが不要なため、固定レート（例：1 USD = 150円）で十分。

### エージェントごとのモデル使い分け
FUTURE_CONCEPTの表は理にかなっている（Opus=壁打ち・レトロ、Sonnet=実行、Haiku=分類）。
現状 `src-tauri/src/ai.rs` ではモデルがハードコードされているため、
`settings.json` に各エージェントのモデルを設定できる仕組みを早めに整備すると良い。
