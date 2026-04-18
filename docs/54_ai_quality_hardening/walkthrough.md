# EPIC54: AI品質向上と保守性改善 Walkthrough

## ステータス

- 状態: `In Progress`
- 作成日: 2026-04-17

## 設計方針

- 本 Epic の目的は機能追加ではなく、AI 関連コードの構造整理と曖昧さの除去である
- 影響範囲が大きいため、`Story 1` / `Story 2` / `Story 3` を明確に分け、各 Story 完了ごとに `cargo test` と `npm run build` を実行してから `task.md` を更新する
- 外部契約に当たる command / event 名の変更は、バックエンドとフロントエンドを同じ段で更新し、必要なら短期的な互換レイヤを残す
- `frontend-core` は参照のみを原則とし、今回の rename に必須な最小範囲以外は触らない

## 初期調査メモ

### `ai.rs` の現状

- `src-tauri/src/ai.rs` は 2847 行
- 少なくとも以下の責務が同居している
  - 共通型
  - transport 解決 / CLI 実行 helper
  - PO アシスタント
  - task generation
  - idea refine
  - inception
  - retro review / synthesis
  - ユニットテスト

### Claude 固有名が残っている共通層

- `src-tauri/src/claude_runner.rs`
- `src-tauri/src/llm_observability.rs` の `ClaudeCliUsageRecordInput`
- Tauri command
  - `get_active_claude_sessions`
  - `execute_claude_task`
  - `kill_claude_process`
- Tauri event
  - `claude_cli_started`
  - `claude_cli_output`
  - `claude_cli_exit`
- フロントエンド購読 / 呼び出し
  - `src/components/terminal/TerminalDock.tsx`
  - `src/components/kanban/TaskCard.tsx`
  - `src/components/project/ScaffoldingPanel.tsx`

### AI 指示文で改善余地がある箇所

- `refine_idea` の API 側 system prompt が `"PO Assist"` のみで、CLI 版より曖昧
- Dev Agent 実行プロンプトは完了条件と自己検証要件がまだ粗い
- `generate_tasks_from_story` は API / CLI で粒度と制約の書き方に差がある
- PO アシスタントは API / CLI の共通核がコード上で分散している

## 実施ログ

### 2026-04-17 Step 0

- `task.md` の Story 2 に、PO 指示の相乗り準備タスク 2 件を追記した
- 本 `walkthrough.md` を新規作成した
- 以降の変更では、各 Story ごとに検証結果と設計判断を追記していく

### 2026-04-17 Step 1: `ai.rs` 分割

- `src-tauri/src/ai.rs` を薄いエントリポイントへ置き換え、実体を以下へ分割した
  - `ai/common.rs`
  - `ai/task_generation.rs`
  - `ai/idea_refine.rs`
  - `ai/inception.rs`
  - `ai/team_leader.rs`
  - `ai/retro.rs`
- 共有型、transport 解決、CLI 実行 helper、JSON parse、usage 記録を `common.rs` へ集約した
- `task_generation` / `idea_refine` / `inception` / `team_leader` / `retro` の `#[tauri::command]` は各責務モジュールへ移し、`lib.rs` の `generate_handler!` は実体モジュールパスを参照する形に更新した
- `ai.rs` 末尾に集中していた unit test は、責務ごとに各モジュールへ分配した

#### 設計判断

- `src-tauri/src/ai.rs` 自体は削除せず、モジュールの入口として残した
  - 理由: `mod ai;` を維持したまま安全に分割でき、影響範囲を最小化できるため
- `team_leader` は依存が多いため、まずロジックをほぼそのまま移し、プロンプト文字列だけ helper 関数へ寄せた
  - 理由: Story 3 で prompt 品質改善を行いやすくするため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 107 passed
- `npm run build`: 成功

### 2026-04-17 Step 2: 共通命名のジェネリック化

- `src-tauri/src/claude_runner.rs` を `src-tauri/src/agent_runner.rs` へ移し、共通実行責務の公開面を `agent_*` 命名へ整理した
- Tauri command を以下へ変更し、`lib.rs` とフロントエンドの invoke 側を同時に追従させた
  - `get_active_agent_sessions`
  - `execute_agent_task`
  - `kill_agent_process`
- Tauri event を `agent_cli_started` / `agent_cli_output` / `agent_cli_exit` へ変更し、`TerminalDock.tsx` と `ScaffoldingPanel.tsx` の購読を更新した
- フロントエンドの共通エラー通知も `claude_error` から `agent_error` へ置き換え、`TaskCard.tsx` と `TerminalDock.tsx` の連携名を揃えた
- `llm_observability.rs` の `ClaudeCliUsageRecordInput` を `CliUsageRecordInput` に改名し、将来の厳密計測用に以下の Optional フィールドを追加した
  - `prompt_tokens`
  - `completion_tokens`
  - `total_tokens`
  - `cached_input_tokens`
- `agent_runner.rs` に `AgentStdoutParser` を追加し、現時点では passthrough 実装を使いつつ、stdout のパース処理を差し替え可能な境界として切り出した

#### 設計判断

- 旧 command / event 名の互換レイヤは今回は残さなかった
  - 理由: 本リポジトリ内の利用箇所を同一コミットで追従でき、互換層が残ると共通層から Claude 固有名を排除する目的に反するため
- stdout パーサーは今の挙動を変えない `PassthroughAgentStdoutParser` を既定にした
  - 理由: 将来 stream-json 専用パーサーを差し込める構造だけ先に作り、現時点の出力内容やターミナル挙動は一切変えないため
- `CliUsageRecordInput` では計測値が未取得のとき従来通り `unavailable` を記録し、将来トークン数が供給された場合のみ `captured` に切り替えるようにした
  - 理由: 既存の集計意味を保ちながら、後続 Epic で厳密計測を段階導入しやすくするため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 107 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-17 Step 3: AI 指示文の品質改善

- Dev Agent 実行プロンプト (`agent_runner.rs`) に、以下を追記した
  - 既存挙動や UI を不必要に変えないこと
  - 完了条件
  - 自己検証
  - 終了時報告の期待値
- `idea_refine.rs` は API / CLI で共有する `IDEA_REFINE_SYSTEM_PROMPT` を導入し、CLI 側だけが詳しい状態を解消した
- `task_generation.rs` は API / CLI で共通の `TASK_GENERATION_SYSTEM_PROMPT` と入力 prompt builder を使う形へ整理し、JSON 契約・優先度・依存関係・自己検証の記述を揃えた
- `team_leader.rs` は API / CLI の双方で使う共通ポリシー / 優先度ルール / 品質ゲートを helper 化し、transport ごとの差分を action 実行方式の違いに限定した
- レトロ review / synthesis prompt には、具体性・重複回避・category / 見出し構造の自己確認条件を追加した
- prompt の意図を固定するため、以下のテストを追加した
  - `ai::idea_refine`
  - `ai::task_generation`
  - `ai::team_leader`
  - `agent_runner`

#### 設計判断

- prompt 改善は「出力契約の明文化」と「API / CLI の共通核の抽出」に絞り、ツール呼び出し方式や返却 JSON スキーマは変更しなかった
  - 理由: 本 Epic は構造整理が目的であり、業務フローや UI 挙動の変化を避けるため
- `team_leader` はファイル全体の再分割までは行わず、まず prompt ポリシーを helper 化した
  - 理由: 一度に責務分割まで進めると回帰リスクが高く、今回の安全第一方針に反するため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 115 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-17 Step 4: `agent_runner.rs` / `team_leader.rs` 再分割

- `src-tauri/src/agent_runner.rs` を薄いルートモジュールへ寄せ、責務を以下へ再配置した
  - `agent_runner/prompting.rs`
  - `agent_runner/session.rs`
  - `agent_runner/lifecycle.rs`
  - `agent_runner/spawn.rs`
- ルート `agent_runner.rs` には以下を残し、外部契約と共有型の入口として機能させた
  - `AgentState`
  - session / event payload の共通型
  - `execute_agent_task`
  - `kill_agent_process`
  - `get_active_agent_sessions`
  - `agent_runner` 関連の unit test
- `prompting.rs` に prompt ファイル生成、CLI 入力準備、response capture 連携を集約した
- `session.rs` に session summary、slot 予約、generic/task 用 session 情報生成を集約した
- `lifecycle.rs` にイベント送出、重複 stdout 抑止、usage 記録、retro 永続化、終了 payload 生成を寄せた
- `spawn.rs` に CLI 起動、timeout kill、標準出力処理、終了時後始末を寄せた
- `src-tauri/src/ai/team_leader.rs` は以下の分割へ移行した
  - `ai/team_leader/heuristics.rs`
  - `ai/team_leader/prompts.rs`
  - `ai/team_leader/plan.rs`
- `heuristics.rs` に backlog mutation 判定、product context の有無判定、provider unavailable 時の応答 helper を集約した
- `prompts.rs` に API / CLI 共通ポリシー、優先度ルール、品質ゲート、contextual backlog 生成 prompt を集約した
- `plan.rs` に execution plan 解析、retry、fallback、plan 適用の処理を寄せた
- 再分割に伴う壊れを避けるため、`agent_retro::AgentRetroPersistInput` への参照名を再確認し、retro 記録導線もそのまま維持した

#### 設計判断

- `agent_runner` は command 層と共有型の公開面をルートに残し、内部実装だけを sibling module へ分けた
  - 理由: `lib.rs` や frontend から見た command 契約を変えずに、プロセス起動・session 管理・prompt 準備・終了処理の責務境界だけを明確化するため
- `team_leader` は prompt / heuristic / plan_apply を分けつつ、`#[tauri::command] chat_with_team_leader` 自体はルートへ残した
  - 理由: transport 切替、DB 更新、fallback 制御の入口を 1 か所に保ち、外部契約を変えずに内部密結合だけを弱めるため
- テストは既存モジュールパスのまま壊さず、責務に近い helper 呼び出しへ差し替える方針を取った
  - 理由: 本 Epic の目的が「機能改変なしの構造整理」であり、テスト仕様を動かさずに分割効果だけを得るため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 115 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-18 Step 5: PO アシスタントの task 分解粒度改善

- `task_generation.rs` の system prompt を強化し、task タイトルと description を自然な日本語で返すことを明示した
- `task_generation.rs` では各 `tasks[*].description` に、以下の 4 項目をこの順番で必ず含めるルールを追加した
  - `やること: ...`
  - `対象範囲: ...`
  - `完了状態: ...`
  - `検証観点: ...`
- task 分解時に、状態/データ、業務ロジック、UI/操作、失敗時の扱い、検証の観点を確認し、受け入れ条件に対する抜け漏れを減らす checklist を追加した
- `team_leader/prompts.rs` の `create_story` 系ルールにも同じ日本語・4項目フォーマットを反映し、PO アシスタント経由の PBI / task 自動登録でも同品質になるよう揃えた
- `task_generation` と `team_leader` の unit test を更新し、日本語出力ルールと詳細 description フォーマットの意図を固定した

#### 設計判断

- schema 変更は行わず、既存の `title` / `description` / `priority` / `blocked_by_indices` のままで task の情報量を増やした
  - 理由: 本 Epic の目的が構造整理と曖昧さの除去であり、フロントエンドや DB 契約を変えずに改善効果を得るため
- 「どこを触るか」ではなく「何をどう進めるか」を task description に書かせる方針にした
  - 理由: 新規プロジェクトのように実装場所がまだ固まっていない段階でも、開発担当が迷わず着手できる粒度へ寄せるため
- プロンプト本文は英語ベースを維持しつつ、出力言語だけ日本語に強く固定した
  - 理由: CLI / API の安定性を保ちながら、日本語運用の現場でそのまま使える task を得るため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 115 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-18 Step 6: 競合後の AI 再実行の安定化

- `worktree.rs` に cleanup helper を追加し、競合後の再実行や手動破棄で使う `remove_worktree` が cleanup 失敗を握りつぶさないようにした
- cleanup では以下を順に試し、最終的に worktree 登録・ディレクトリ・task branch の残存有無を検証する形へ変えた
  - `git worktree remove --force`
  - ローカル worktree ディレクトリ削除
  - `git worktree prune`
  - `git branch -d` / `git branch -D`
- cleanup 後も branch や worktree が残っている場合は `removed` 扱いにせず、backend でエラーを返すようにした
- `agent_runner.rs` では session 予約を worktree 復旧・作成の後ろへ移し、競合 rerun の途中で失敗しても `CLI プロセスはすでに起動中` が残留しないようにした
- worktree の unit test は helper ベースへ更新し、branch と plain directory だけが残る stale artifact の cleanup ケースも追加した

#### 設計判断

- rerun の詰まりは「worktree cleanup 不完全」と「session 先取り」の複合で起きていたため、どちらか片方ではなく両方を最小範囲で修正した
  - 理由: branch だけ残るケースを直しても session が残れば再実行できず、session だけ直しても stale branch が残れば create_worktree が再度失敗するため
- `remove_worktree` は best-effort ではなく、cleanup 完了を確認してから `removed` 状態へ進める方針にした
  - 理由: 失敗したのに DB だけ removed へ進むと、UI では再実行可能に見えて backend だけ壊れている状態を生みやすいため
- `execute_agent_task` の session 予約タイミングは、既存の command 契約を変えずに worktree 準備後へ移した
  - 理由: 競合後 rerun の前段失敗で session がリークする問題を、公開面を変えずに解消するため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 116 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-18 Step 7: Scaffolding 後の Node 依存 bootstrap と worktree 共有の補強

- `scaffolding.rs` に package.json 検出と package manager 推定の helper を追加し、CLI scaffold / AI scaffold の完了後に `npm install` などの依存導入を自動実行するようにした
- 依存導入はルート `package.json` を優先し、root が workspace 構成でない場合は root scripts に含まれる `--prefix <dir>` も拾って、`frontend/package.json` のような配下パッケージも bootstrap する
- install 実行は `scaffold_output` に stdout / stderr をストリームし、失敗時は exit code つきで scaffolding 自体を失敗扱いにするようにした
- `worktree.rs` の `link_node_modules` は root 直下だけでなく、既存プロジェクト配下にある `node_modules` を再帰的に検出して worktree 側へ link / junction を張る形へ広げた
- worktree cleanup 時の `remove_worktree_node_modules_link` も複数の `node_modules` パスを辿って外すようにし、Scaffolding 後に生成された依存を task worktree 破棄で失わない前提を強めた
- unit test は package manager 推定、`--prefix` 検出、install plan discovery、nested `node_modules` link のケースを追加した

#### 設計判断

- 依存導入は「必要なら後で手動実行」ではなく Scaffolding 完了フローの一部として扱った
  - 理由: root `node_modules` が無いまま worktree を作ると共有 link が張られず、task 側で入れた依存が worktree 破棄と一緒に消えるため
- root が workspace 構成なら root install のみ、非 workspace なら `--prefix` 配下も install する方針にした
  - 理由: workspace プロジェクトでは root install だけで十分な一方、今回の preview 規約のような `npm --prefix frontend run dev` 構成では配下 package の bootstrap も必要になるため
- install 実行は PTY ではなく backend の shell process で流し、`scaffold_output` に転送する形を採った
  - 理由: `npm install` は 30 秒を超える可能性があり、既存 PTY helper の固定 timeout に乗せるより専用ストリーミングの方が安全だったため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 119 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-18 Step 8: Preview 差分コメント付きの Dev エージェント再実行

- `TaskCard.tsx` の Review / Preview セクションに「コメントを付けて再開発」導線を追加し、Preview で見つかった期待との差分を textarea で入力できる modal を実装した
- modal からの再実行は既存の `execute_agent_task` command をそのまま使い、`additionalContext` に preview feedback を構造化した文面を渡す形にした
- Preview feedback の文面は「期待との差分」「修正時の注意」「最終報告で自己確認すべき点」を含むため、追加の IPC や DB 変更なしで Dev エージェントへ意図を伝えられる
- `agent_runner/prompting.rs` では、追加コンテキストにレビュー指摘や期待との差分が入る場合はそれを最優先で解消し、自己検証でも確認するよう明文化した
- `agent_runner.rs` の prompt test は、この追加文言が prompt に残ることを確認する形へ更新した

#### 設計判断

- 新しい rerun command は追加せず、既存の `execute_agent_task(additional_context)` を再利用した
  - 理由: backend の公開契約を増やさず、競合 rerun と同じ再開発経路へ preview feedback も乗せた方が安全だったため
- コメントは DB 保存ではなく prompt への追加コンテキストとして扱った
  - 理由: 今回必要なのは「Preview で見つけた差分をその場で Dev エージェントへ伝えて再開発すること」であり、永続メモ機能まで広げると仕様変更が大きくなるため
- prompt 側に「レビュー指摘を優先課題として扱う」明文化を追加した
  - 理由: UI から渡した preview feedback が単なる参考情報として埋もれず、完了条件に近い優先修正事項として解釈されるようにするため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 119 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-18 Step 9: Windows preview 残留プロセスの cleanup 補強

- `preview.rs` に preview 停止 helper を追加し、Windows では `taskkill /PID /T /F` で `cmd -> npm -> concurrently/node` のプロセスツリーごと停止するようにした
- `PreviewState` に task の preview session が残っていない場合でも、DB の `preview_pid` を fallback に使って stale preview process を停止できる `stop_server_or_fallback_pid` を追加した
- `worktree.rs` では `remove_worktree` / `merge_worktree` / `stop_preview_server` がこの fallback stop を使うように変え、アプリ再起動や state ずれがあっても merge/remove 後の残留を片付けやすくした
- `start_preview_server` の起動前にも stale preview cleanup を入れ、前回 preview の残留がある場合でも先に掃除してから新しい preview を起動するようにした
- unit test は preview fallback stop の no-op ケースと、DB record の `preview_pid` 変換 helper を追加した

#### 設計判断

- 問題は stop の呼び忘れではなく「Windows で親 process だけ kill していたこと」だったため、merge 導線より先に preview 停止実装を補強した
  - 理由: `merge_worktree` 自体は従来から `stop_server` を呼んでいたが、`cmd /C npm run dev` 配下の子孫 process が残ると次 task の初回 preview が blocked されるため
- fallback cleanup は DB の `preview_pid` を再利用し、新しい preview API や専用テーブルは増やさなかった
  - 理由: 既存の worktree record に必要な情報は揃っており、まずは最小差分で stale process を止めるほうが安全だったため
- `start_preview_server` でも stale cleanup を走らせた
  - 理由: 過去に残った preview process がすでに存在する場合、merge/remove 修正だけでは次の初回 preview 失敗を防ぎ切れないため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 121 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-18 Step 10: Dev Agent 完了後の package manifest 変更に対する自動 install

- Scaffolding に閉じていた Node 依存導入 helper を `node_dependencies.rs` へ抽出し、Scaffolding と Dev Agent 完了処理の両方から再利用できるようにした
- `agent_runner/lifecycle.rs` では worktree 差分の収集を `WorktreeChangeSet` へ整理し、`package.json` / lockfile 変更を検知した場合だけ自動 install を走らせるようにした
- 自動 install は worktree 上の root `package.json` を起点に、非 workspace 構成なら `--prefix <dir>` で参照される `frontend/package.json` などもまとめて再同期する
- install 実行ログは `agent_cli_output` へ流すため、Terminal Dock から「どの package で何を実行したか」「stdout / stderr に何が出たか」をそのまま確認できる
- `worktree.rs` には worktree path から project root を復元し、既存の shared `node_modules` link / junction を張り直す helper を追加した
- 依存再同期に失敗した場合は task を `Review` へ進めず、`CLI は完走したが依存再同期に失敗した` という明示エラーで止めるようにした
- unit test は `node_dependencies.rs` に追加し、manifest 変更検知、package manager 推定、`--prefix` 抽出、install plan discovery を固定した

#### 設計判断

- Dev Agent 後の install は main 側 project root ではなく worktree 側で実行する方針にした
  - 理由: 新しく変更された `package.json` は merge 前の時点では worktree にしか存在せず、main 側で install すると最新 manifest を参照できないため
- install の実行対象は「manifest が変わったら root + 必要な `--prefix` package をまとめて再同期」に寄せた
  - 理由: `frontend/package.json` だけ更新されたケースでも preview は root script 経由で起動されることが多く、個別判定より既存 `discover_node_install_plans` の方が安全だったため
- shared `node_modules` link がある場合はそれを再利用し、無い場合でも worktree の現行 manifest で install できる構造を優先した
  - 理由: Scaffolding 後の新規 project では shared link が効く一方、既存 project でも少なくとも当該 worktree の preview 失敗を減らすことを優先したため
- install 失敗は単なるログではなく task 完了失敗として扱った
  - 理由: `package.json` 更新後に依存が解決できていない状態で `Review` へ進むと、Preview 初回失敗を backend が見逃す形になるため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 122 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-19 Step 11: Preview 起動前の依存ヘルスチェックと self-heal

- `node_dependencies.rs` に preview preflight 用の helper を追加し、`npm run dev` / `npm --prefix frontend run dev` / `yarn dev` などの script を解析して local binary 不足を検知できるようにした
- 検知対象は `node_modules` 自体の欠落だけでなく、`concurrently` / `vite` / `next` など script 冒頭で使う local binary が `node_modules/.bin` に存在しないケースも含めた
- nested script (`npm:dev:web`) と `--prefix frontend run dev` も再帰的に辿るため、root script から起動する monorepo / concurrently 構成でも不足 binary を拾えるようにした
- `worktree.rs` の `start_preview_server` では preview 起動前に shared `node_modules` link を張り直し、問題が見つかった場合だけ install を走らせる self-heal を追加した
- install は `NodeInstallOutputTarget::Silent` で backend log にだけ流し、通常の preview 起動では余計な UI イベントを増やさずに修復だけ行うようにした
- unit test は `node_dependencies.rs` に追加し、root `concurrently` + nested `frontend/vite` 構成で不足時のみ issue を返すことを固定した

#### 設計判断

- Preview 起動前の self-heal は「毎回 install」ではなく「不足が検知されたときだけ install」にした
  - 理由: preview のたびに `npm install` を走らせると待ち時間が大きく、lockfile 更新リスクも増えるため
- 不足判定は単なる `node_modules` 有無だけでなく script 解析を加えた
  - 理由: root `node_modules` は存在していても `concurrently` だけ未導入、あるいは `frontend/node_modules` はあるが `vite` が無い、といった今回の実例を existence check だけでは拾えないため
- Preview 側の self-heal は worktree の現行 manifest に対して実行し、shared `node_modules` があればそこへ反映される構造を維持した
  - 理由: merge 前の task 変更を main 側ではなく worktree 側の最新状態で確認しつつ、既存の共有前提も崩さないため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 124 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-19 Step 12: `spawn` / `plan` の再分割で密結合をさらに緩和

- `agent_runner/spawn.rs` をフォルダモジュールへ置き換え、以下へ責務分割した
  - `agent_runner/spawn/mod.rs`: `execute_prompt_request` / `execute_cli_prompt_task` の入口
  - `agent_runner/spawn/completion.rs`: 終了理由決定、retro persist、usage 記録、exit payload 生成
  - `agent_runner/spawn/timeout.rs`: 180 秒 timeout の後始末
  - `agent_runner/spawn/windows.rs`: Windows の process spawn と stdout/stderr reader
  - `agent_runner/spawn/unix.rs`: PTY ベースの spawn と wait loop
- `ai/team_leader/plan.rs` もフォルダモジュールへ置き換え、以下へ責務分割した
  - `ai/team_leader/plan/mod.rs`: 再 export のみを持つ入口
  - `ai/team_leader/plan/apply.rs`: action / operation の DB 適用
  - `ai/team_leader/plan/fallback.rs`: provider fallback、CLI fallback、plan parse
- `apply_team_leader_action` / `apply_team_leader_operations` を分離し、PO アシスタントの DB mutation と fallback 生成が同一ファイルに混在しないようにした
- これにより、platform 差分や fallback 分岐を単体で追いやすくし、今後の unit test 追加位置も明確にした

#### 設計判断

- `agent_runner` は root command 契約を変えず、`spawn` だけをさらに分解する方針にした
  - 理由: `execute_agent_task` / `kill_agent_process` の公開面は安定させたまま、最も密度が高かった process lifecycle 部分だけを薄くするため
- Windows / Unix の process spawn は別ファイルへ完全分離した
  - 理由: `portable_pty` と `std::process` の分岐を 1 ファイルに置くとレビュー観点とテスト観点が混ざりやすく、保守性を落としていたため
- `team_leader/plan` は「plan を作る責務」と「plan を適用する責務」を分けた
  - 理由: fallback prompt の調整と DB 反映の修正が同一ファイルで競合すると、PO アシスタント周辺の変更コストが高くなるため
- `AnalyticsTab.tsx` の `claude_cli` 補正は今回は触らず残した
  - 理由: 旧観測データ互換の吸収ロジックであり、`frontend-core` 配下の変更を伴うため、別 Story でデータ移行方針と合わせて扱う方が安全だったため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 124 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-19 Step 13: `agent_retro` の provider 固有 parser を分離し、stale naming を整理

- `src-tauri/src/agent_retro.rs` を `src-tauri/src/agent_retro/` ディレクトリへ置き換え、capture 本体と Claude stream-json parser を分離した
- `agent_retro/mod.rs` には以下を残した
  - `AgentRetroCapture`
  - `AgentRetroPersistInput`
  - generic な `CaptureMutation` 適用
  - retro run persist と共通 utility
- `agent_retro/claude_stream.rs` に Claude CLI の structured stream parser を切り出し、`AppendReasoning` / `SetFinalAnswer` / `ResolveToolResult` などの mutation を返す構造へ置き換えた
- これにより `AgentRetroCapture` 本体から `handle_claude_*` 系メソッドを外し、provider 固有処理は専用モジュールへ閉じ込めた
- あわせて、backend 側の `Claude` 命名を再監査し、共通責務に残っていた stale naming は解消済みであることを確認した
  - 依然残る `Claude` 表記は `cli_runner/claude.rs` や `.claude/settings.json` 生成など、provider 固有または互換用途のものだけ

#### 設計判断

- `agent_retro` は parser strategy を導入しつつ、外部契約は `AgentRetroCapture::new/ingest_chunk/finalize` のまま維持した
  - 理由: retro persist 導線や `agent_runner` 側の呼び出しを変えずに、内部の provider 固有ロジックだけを分離するため
- mutation ベースで parser と capture をつなぐ形にした
  - 理由: parser 側が `reasoning_log` や `tool_events` の内部表現を直接触らないことで、今後 Gemini / Codex 向けの structured parser を追加しやすくするため
- `AnalyticsTab.tsx` の `claude_cli` 補正は今回も変更しなかった
  - 理由: `frontend-core` 配下の過去データ互換ロジックであり、別 Story で観測データ移行方針とセットで扱う方が安全だったため

#### 検証結果

- `cargo test --manifest-path src-tauri/Cargo.toml`: 124 passed
- `npm run build`: 成功
- `vite` の chunk size warning は継続だが、今回差分による新規エラーではないため保留

### 2026-04-19 Step 14: PO による最終回帰確認と Epic 54 クローズ

- PO により以下の手動確認が完了し、主要導線が変更前と同等以上であることを確認した
  - Dev Agent 実行 / 停止 / ターミナル表示
  - PO アシスタントの主要フロー（idea refine / task generation / team leader）
  - レトロレビュー / KPT 合成
  - Scaffolding の CLI 実行イベント連携
- `task.md` の Story 5 と完了条件をすべて完了へ更新した
- Epic 54 は「機能改変ではなく構造整理と曖昧さ除去を安全に完遂した Epic」としてクローズした

#### 設計判断

- 手動回帰確認は PO の実機確認結果を正式な完了根拠として採用した
  - 理由: 本 Epic の最終受け入れ条件は、実利用導線が壊れていないことをプロダクトオーナー観点でも確認することにあったため
- BACKLOG.md は精査したが、Epic 54 で完了済みとして削除すべき項目は存在しなかったため変更しなかった
  - 理由: 現在残っている backlog は stream-json 本実装や観測データ移行など、今回の Epic では準備までに留めた将来課題のみだったため

#### 検証結果

- PO による手動回帰確認: 完了
- 自動検証 (`cargo test --manifest-path src-tauri/Cargo.toml`, `npm run build`): 直近成功結果を維持

## 追加調査メモ

- `src-tauri/src/agent_runner.rs` のルートは 422 行前後で安定し、`spawn` も `completion / timeout / windows / unix` へ分割済み
  - 次の分割候補をあえて挙げるなら、Windows 側の stdout/stderr reader を `stream_reader` helper へ寄せる余地がある
- `src-tauri/src/ai/team_leader.rs` のルートは 291 行前後で安定し、`plan` も `apply / fallback` へ分割済み
  - 次の分割候補をあえて挙げるなら、`apply.rs` の action handler を command ごとの table へ寄せる余地がある
- `src/components/ui/AnalyticsTab.tsx` には、旧観測データ互換のため `claude_cli` を `gemini/codex` へ読み替える補正ロジックが残っている
  - これは共通命名負債というより過去データ吸収の互換処理なので、別 Story で観測データ移行方針と合わせて整理したい
- `agent_retro` の provider 固有 parser 分離は完了した
  - 将来 Gemini / Codex 側で同様の構造化出力を扱う場合は、`CaptureMutation` を返す parser を追加すれば拡張できる
