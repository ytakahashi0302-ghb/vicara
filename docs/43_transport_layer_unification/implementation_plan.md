# Epic 43: PO アシスタント Provider / Transport 信頼性改善 実装計画

## ステータス

- 状態: `Ready`
- 実装開始条件: Epic 42 完了
- 更新日: 2026-04-10

## Epic の目的

Epic 42 で PO アシスタントは CLI / API の選択に対応したが、実運用の観点では provider / transport ごとの安定性に差が残っている。Epic 43 では「設定できる」状態から一歩進めて、「どの組み合わせでも期待通りに動く」「失敗時も理由が分かる」状態まで引き上げる。

加えて、Claude API では実行自体は成功しても、既存実装を踏まえず重複 backlog を作るケースが確認された。これは実行 transport の問題ではなく、PO アシスタントへ渡すコンテキストの精度不足と重複防止ガード不足が原因であるため、Epic 43 では reliability の一部として同時に扱う。

## 現在の確認結果

2026-04-10 時点の手動確認結果は以下。

| 組み合わせ | 状態 | 補足 |
|-----------|------|------|
| Claude CLI | ○ | PO アシスタントで backlog 作成まで確認済み |
| Claude API | ○ | 実行自体は成功。ただし context 不足により既存実装と重複する backlog 提案あり |
| Gemini CLI | × | headless / trust / cwd 周りの影響が疑われ、タイムアウト継続 |
| Gemini API | × | 503 UNAVAILABLE が断続的に発生 |
| Codex CLI | ? | 未検証 |
| OpenAI API | ? | 未検証 |

## スコープ

### 対象ファイル（変更候補）
- `src-tauri/src/ai.rs` — provider / transport ごとのリトライ、エラー整形、PO プロンプト改善
- `src-tauri/src/db.rs` — `build_project_context()` の見直し、完了済み実装サマリの追加
- `src-tauri/src/ai_tools.rs` — 重複 backlog 抑止、既存 story への寄せ方改善
- `src-tauri/src/rig_provider.rs` — API provider ごとの観測・リトライ判断整理
- `src-tauri/src/cli_runner/gemini.rs` — Gemini CLI の起動条件再設計
- `src/components/ui/GlobalSettingsModal.tsx` — 必要なら provider 別の注意文や状態表示を追加
- `docs/43_transport_layer_unification/task.md`
- `docs/43_transport_layer_unification/walkthrough.md`
- `docs/43_transport_layer_unification/handoff.md`

### 対象外
- Team Settings 全体の transport 統一 UI リデザイン
- Dev エージェント側の transport 抽象再設計
- 新しい provider 追加

## 実装方針

### 1. provider / transport 検証マトリクスを先に固定する

Epic 43 では実装前に「何が通れば完了か」を曖昧にしない。PO アシスタントの 4 機能すべてを一度に全組み合わせで見るのではなく、まず以下の代表シナリオで matrix を埋める。

1. `refine_idea`
2. `generate_tasks_from_story`
3. `chat_inception`
4. `chat_with_team_leader` による backlog 作成

特に `chat_with_team_leader` は DB 更新を伴うため、以下を分けて記録する。

- 実行成功 / 失敗
- DB 反映の有無
- 最終返信の正常性
- provider 固有エラーの有無

### 2. Gemini CLI の停止要因を再観測可能にする

現状の Gemini CLI は timeout でしか失敗理由が見えず、再現時の観測情報が不足している。Epic 43 では以下を行う。

- headless 起動時の `stdout` / `stderr` / exit status を短く要約して返せるようにする
- timeout 前に取得済みの部分出力があればログとして保存する
- `cwd`、`--prompt`、stdin、trust folder の影響を切り分けられるようにする

必要なら、Gemini CLI だけ「実行用 cwd」と「コンテキスト取得元 local_path」を分離したまま運用する。

### 3. Gemini API の 503 は一時障害と恒常障害を分ける

Epic 42 では 503 に対する再試行と通常返答化を入れたが、依然として「未作成で終わる」ケースがある。Epic 43 では以下を整理する。

- 503 / UNAVAILABLE の再試行条件を provider ごとに明文化する
- `create_story_and_tasks` の実行前失敗 / 実行後失敗を区別する
- 部分成功時の UI 表示を provider 非依存で統一する

### 4. Claude API の重複 backlog 提案は context 精度の問題として扱う

調査結果から、Claude API が DB 設計系 backlog を出した主因は以下。

- `build_project_context()` が `archived = 0` の story / task しか渡していない
- スプリント完了時に Done task が archive されるため、実装済み事実が context から抜け落ちる
- `ARCHITECTURE.md` が PostgreSQL 前提のままで、SQLite 移行の現状とズレている
- `create_story_and_tasks` に重複 story 抑止のガードがない

Epic 43 では、PO アシスタント用コンテキストに以下の層を追加することを検討する。

- 完了済み story / task の要約
- 直近で完了した主要実装一覧
- 「既存 story に統合すべき候補」の判定材料

### 5. tool 側に最後の防波堤を置く

プロンプト改善だけに依存せず、`create_story_and_tasks` 実行直前にも安全策を入れる。

候補:

- 新規 story 作成前に、既存 story のタイトル類似度をチェック
- 類似 story がある場合は `target_story_id` を要求する、または失敗として返す
- 同一テーマの未完了 story が存在する場合は、新規作成ではなく task 追加を促す

## テスト方針

### 自動テスト

- `cargo test --manifest-path src-tauri/Cargo.toml`
- `npm run build`
- context 構築ロジックの unit test
- provider / transport ごとのエラー整形関数の unit test
- 重複 backlog 抑止ロジックの unit test

### 手動テスト

以下の matrix を最低限埋める。

1. Claude CLI
   - backlog 作成
   - task 生成
2. Claude API
   - backlog 作成
   - 既存実装済み機能に対して重複 story を作らないこと
3. Gemini CLI
   - `refine_idea`
   - `chat_with_team_leader`
4. Gemini API
   - 503 発生時の再試行 / 通常返答化
5. Codex CLI
   - `refine_idea`
6. OpenAI API
   - `chat_with_team_leader`

### 完了条件

- 少なくとも Claude CLI / Claude API / OpenAI API / Codex CLI の基本シナリオが成功する
- Gemini CLI / Gemini API は、成功するか、失敗理由が UI 上で明確に分かる状態になる
- Claude API で既存実装と重複する DB 設計 backlog を再作成しない
