# Epic 43: PO アシスタント Provider / Transport 信頼性改善 タスクリスト

## ステータス

- 状態: `Ready`
- 着手条件: Epic 42 完了
- 更新日: 2026-04-10

## 概要

Epic 42 で追加した PO アシスタントの CLI / API 選択対応を、実運用に耐える品質まで引き上げる。provider / transport ごとの成功条件と失敗理由を明確にし、Gemini 系の不安定さと Claude API の context 精度問題を解消する。

## 現状マトリクス

- Claude CLI: `○`
- Claude API: `○` ただし context 不足により重複 backlog 作成あり
- Gemini CLI: `×`
- Gemini API: `×`
- Codex CLI: `?`
- OpenAI API: `?`

## 実行順序

### 1. 現状再現と観測ログ整備
- [ ] provider / transport ごとの再現シナリオを固定する。
- [ ] `refine_idea` / `generate_tasks_from_story` / `chat_inception` / `chat_with_team_leader` の代表ケースを決める。
- [ ] 成功 / 失敗 / DB 反映 / 最終返信の観測項目を共通フォーマットで記録できるようにする。

### 2. Gemini CLI の headless 実行デバッグ
- [ ] timeout 時に原因調査に必要な `stdout` / `stderr` / exit status / cwd を把握できるようにする。
- [ ] trust folder / 実行ディレクトリ / `--prompt` / stdin の切り分けを行う。
- [ ] `chat_with_team_leader` まで正常完了する構成、または明確な失敗メッセージ返却を実現する。

### 3. Gemini API の安定化
- [ ] 503 / `UNAVAILABLE` の再試行条件を見直す。
- [ ] tool 実行前失敗 / tool 実行後失敗 / 部分成功を区別して扱う。
- [ ] UI 上で「未作成」「部分成功」「成功」の違いが分かる返答に統一する。

### 4. PO コンテキスト精度の改善
- [ ] `build_project_context()` に、完了済み story / task の要約を含める方針を決める。
- [ ] `ARCHITECTURE.md` / `PRODUCT_CONTEXT.md` / backlog の優先順位を見直す。
- [ ] 既存実装済みの DB 設計や一覧・詳細表示機能を再提案しないための文脈を追加する。

### 5. 重複 backlog 防止
- [ ] `create_story_and_tasks` 実行前に、既存 story との類似チェックを入れる方針を決める。
- [ ] 類似 story がある場合は、新規作成ではなく task 追加へ寄せるか、明示的に失敗させる。
- [ ] 抽象依頼時でも既存 backlog を優先活用するルールを system prompt と tool 側の両方に反映する。

### 6. 未検証 provider / transport の確認
- [ ] Codex CLI の `refine_idea` と `chat_with_team_leader` を検証する。
- [ ] OpenAI API の `refine_idea` と `chat_with_team_leader` を検証する。
- [ ] 成否と制約を setup / handoff に反映する。

### 7. 動作確認
- [ ] Claude CLI で backlog 作成が成功すること。
- [ ] Claude API で既存実装と重複しない backlog を作成できること。
- [ ] Gemini CLI で少なくとも 1 機能は timeout せず完走すること、または UI で原因が分かること。
- [ ] Gemini API で 503 発生時の挙動が一貫していること。
- [ ] Codex CLI の基本シナリオが確認できること。
- [ ] OpenAI API の基本シナリオが確認できること。
