# EPIC50 実装計画

## 概要

Claude CLIの出力がDevターミナルにストリーミングされない問題を調査・修正する。このEPICは他のEPIC（47-49）と独立して並行実施可能。

## 現状整理

### 共通ストリーミング基盤（claude_runner.rs）

Windows環境では全CLI（Claude/Gemini/Codex）が同一の `spawn_agent_process` 関数で実行される:

```rust
// L752-918: spawn_agent_process (Windows版)
// 1. std::process::Command でプロセス起動（stdout/stderr piped）
// 2. stdoutリーダースレッド: 1024バイトずつ読み取り → emit("claude_cli_output")
// 3. stderrリーダースレッド: 同上
// 4. waitスレッド: プロセス終了待ち → emit("claude_cli_exit")
```

重複抑制ロジック（L384-431）:
```rust
fn should_suppress_duplicate_output(...) -> bool {
    // 750ms以内に同一内容が来た場合にsuppress
}
```

### Claude CLI固有の構成（cli_runner/claude.rs）

```rust
fn build_args(&self, prompt: &str, model: &str, cwd: &str) -> Vec<String> {
    vec!["-p", prompt, "--model", model, "--permission-mode", "bypassPermissions",
         "--add-dir", cwd, "--verbose"]
}
// prepare_invocation: デフォルト実装（何もしない）
// env_vars: デフォルト実装（空）
```

### Gemini CLI（参考: npm shim解決済み）

```rust
fn prepare_invocation(...) -> Result<(PathBuf, Vec<String>), String> {
    // Windowsでは .cmd shimを検出し、node.exe + gemini.js に書き換える
    resolve_windows_npm_shim(command_path)?
}
```

### 差異分析

| 項目 | Claude | Gemini | Codex |
|------|--------|--------|-------|
| npm shim解決 | **未実装** | 実装済み | 実装済み |
| 起動方式 | `.cmd` shim経由 | `node.exe` 直接 | `node.exe` 直接 |
| stdin使用 | なし | なし | あり（prompt渡し） |
| 特殊フラグ | `--verbose` | `--yolo` | `--full-auto` |

## 推定原因

### 原因候補 1: `.cmd` shimによるプロセスパイプの断絶（最有力）

Gemini/Codexは `prepare_invocation` でnpm shimを解決し、`node.exe` を直接起動している。Claude CLIだけが `.cmd` shim経由で起動されているため:

- `.cmd` shim → `cmd.exe /C node.exe claude.js ...` のようにサブプロセスが挟まる
- piped stdout/stderrが `cmd.exe` のバッファリングの影響を受ける
- `cmd.exe` が終了するまでstdoutが flush されない可能性がある

### 原因候補 2: Claude CLIのstdoutバッファリング

- Node.jsのstdoutはTTY接続時はライン単位flush、パイプ接続時はブロック単位flush
- `.cmd` shim経由だと追加のバッファリング層が入る

### 原因候補 3: 重複抑制による過剰フィルタ

- 可能性は低いが、Claude CLIの出力パターンが重複抑制に引っかかっている可能性

## 実施ステップ

### Step 1: デバッグログ追加で原因切り分け

`claude_runner.rs` のstdout/stderrリーダースレッドにログ追加:

```rust
// L830-857 のstdoutリーダー内
Ok(n) => {
    let output = String::from_utf8_lossy(&buf[..n]).to_string();
    log::debug!("[STREAM] task={} stdout chunk: {} bytes", tid_out, n);
    if should_suppress_duplicate_output(&recent_output_out, &output) {
        log::debug!("[STREAM] task={} SUPPRESSED duplicate", tid_out);
        continue;
    }
    log::debug!("[STREAM] task={} EMITTING: {:?}", tid_out, &output[..output.len().min(100)]);
    // ... emit ...
}
```

これでstdoutリーダーが全くデータを受信していないのか、受信しているが抑制されているのかが分かる。

### Step 2: Claude CLIの `prepare_invocation` 実装

Gemini Runnerの `resolve_windows_npm_shim` パターンに倣い、Claude Runner用のnpm shim解決を追加:

```rust
// cli_runner/claude.rs
fn prepare_invocation(
    &self,
    command_path: &Path,
    args: Vec<String>,
) -> Result<(PathBuf, Vec<String>), String> {
    #[cfg(windows)]
    {
        if let Some((node_path, mut prefix_args)) = resolve_windows_npm_shim(command_path)? {
            prefix_args.extend(args);
            return Ok((node_path, prefix_args));
        }
    }
    Ok((command_path.to_path_buf(), args))
}
```

npm shim解決のヘルパー関数 `resolve_windows_npm_cli_invocation` は既に `cli_runner/mod.rs` に存在する。Claude CLIのnode_modules内パスを特定する必要がある:

```rust
#[cfg(windows)]
fn resolve_windows_npm_shim(command_path: &Path) -> Result<Option<(PathBuf, Vec<String>)>, String> {
    super::resolve_windows_npm_cli_invocation(
        command_path,
        "claude",
        &["node_modules", "@anthropic-ai", "claude-code", "cli.js"],  // 要確認
        &[],  // prefix args不要
    )
}
```

**注意:** `@anthropic-ai/claude-code` パッケージ内のエントリポイントのパスは実際のインストール構造を確認して決定する。

### Step 3: 出力フォーマットフラグの検討

Claude CLI に `--output-format stream-json` のようなストリーミング用フラグがある場合:
- `build_args` に追加する
- パースは不要（TerminalDockはraw text表示）

ただしStep 2のnpm shim解決で問題が解決する可能性が高いため、Step 2の結果を見てから判断する。

### Step 4: 検証

1. Claude CLIでタスク実行し、Devターミナルにリアルタイム出力が表示されることを確認
2. Gemini/Codexで同様に確認（回帰テスト）
3. デバッグログを確認し、stdout chunkが正常にemitされていることを確認
4. 長時間タスクで出力が途切れないことを確認

## リスクと対策

### リスク 1: Claude CLIのnode_modulesパスが異なる

- `which claude` / `where claude` でインストール先を確認
- グローバルインストール時のパス構造を調査
- パスが見つからない場合はフォールバックで `.cmd` shim経由を維持

### リスク 2: npm shim解決後もバッファリングが残る

- `NODE_OPTIONS` 環境変数でstdoutバッファリングを制御する
- `env_vars()` に追加: `("NODE_NO_WARNINGS", "1")` 等

### リスク 3: Windowsでの `cmd.exe` 挙動の違い

- `CREATE_NO_WINDOW` フラグの影響を確認
- 必要に応じて `Command` の `.creation_flags()` を調整

## テスト方針

### 自動テスト

- `ClaudeRunner` の `prepare_invocation` ユニットテスト（Geminiのテストパターンに従う）
- npm shimの解決パスが正しいことを確認

### 手動確認

- Claude CLIでタスクを実行し、Devターミナルでリアルタイム出力を確認
- Gemini/Codexで同じタスクを実行し、ストリーミングが維持されていることを確認
- タスク完了時に `claude_cli_exit` イベントが正しく発火することを確認

## 成果物

- `src-tauri/src/cli_runner/claude.rs`（`prepare_invocation` 追加）
- `src-tauri/src/claude_runner.rs`（デバッグログ追加、必要に応じて調整）

---

## 2026-04-15 スコープ拡張: レトロ用 CLI 実行ログ保存

### 追加背景

ストリーミングが安定したことで、次は「レトロスペクティブで何を振り返れるか」を整備する。  
ただし生の `stream-json` 全保存は容量効率が悪く、可読性も低いため、**レトロに必要な最小構造化データだけを保存する**方針とする。

### 保存方針

各 CLI 実行ごとに、以下を保存対象とする。

- 実行メタ情報
  - project / sprint / task / role / cli_type / model
  - started_at / completed_at / duration / success / error_message
- レトロ本文
  - `reasoning_log`: thinking または実行中の判断ログ
  - `final_answer`: 最終回答
  - `changed_files_json`: 実質差分ファイル一覧
- ツール履歴
  - `tool_name`
  - `status`
  - `summary`

### DB 設計案

#### 1. `agent_retro_runs`

1 実行 1 レコードの集約テーブル。

- `id`
- `project_id`
- `task_id`
- `sprint_id`
- `source_kind`
- `role_name`
- `cli_type`
- `model`
- `started_at`
- `completed_at`
- `duration_ms`
- `success`
- `error_message`
- `reasoning_log`
- `final_answer`
- `changed_files_json`
- `tool_event_count`
- `created_at`

#### 2. `agent_retro_tool_events`

ツール利用の明細テーブル。1 run に対して 0..N 件。

- `id`
- `run_id`
- `sequence_number`
- `tool_name`
- `status`
- `summary`
- `created_at`

### 収集戦略

#### Claude CLI

- 既存の `stream-json` をバックエンド側でもパースし、`thinking_delta` / assistant text / tool_use を構造化抽出する。
- ターミナル表示用の整形とは別に、レトロ保存用の集約バッファを持つ。

#### Gemini CLI

- まずは現行統合を壊さないことを優先し、出力本文を集約して `reasoning_log` / `final_answer` 候補として保存する。
- structured な tool event が安定取得できるバージョンでは parser を差し替えられるよう、DB は CLI 非依存にしておく。

#### Codex CLI

- 既存の `--output-last-message` capture を `final_answer` に流用する。
- 実行中ログは `reasoning_log` に集約保存する。
- 将来的な `--json` structured capture に差し替え可能なように parser 入口を分離する。

### 容量対策

- `reasoning_log` は保存前に整形し、最大文字数を制限する。
- `final_answer` も上限を設ける。
- 生の `stream-json` / 全 stdout 全文 / ツール出力全文は保存しない。
- 保存対象は「レトロで振り返る判断材料」に限定する。

### 実装ステップ

#### Step 5: DB スキーマ追加

- migration を追加し `agent_retro_runs` / `agent_retro_tool_events` を作成する。
- `db.rs` に insert / select 用 struct と helper を追加する。

#### Step 6: 実行中集約バッファの追加

- `claude_runner.rs` にセッション単位の retro capture state を追加する。
- stdout / stderr 読み取りごとに CLI 別 parser へ chunk を渡す。

#### Step 7: 実行完了時の永続化

- プロセス終了時に capture state を確定し、差分ファイル一覧を付与して DB 保存する。
- 保存失敗時でもタスク本体の完了処理は阻害しない。

### リスク

#### リスク 4: CLI ごとの出力形式差

- Claude は structured 抽出、それ以外はまず best-effort 保存で開始する。
- parser は CLI ごとに分離し、後から置き換え可能にする。

#### リスク 5: 保存サイズの増加

- 文字数上限を導入する。
- 生データではなく整形済みテキストのみ保存する。

### テスト方針（追加）

#### 自動テスト

- retro capture parser のユニットテスト
- DB insert helper のユニットテスト
- 文字数上限制御と changed_files 保存のテスト

#### 手動確認

- Claude 実行後に 1 run / N tool events が DB に記録されること
- Gemini / Codex 実行でも run レコードが残ること
- reasoning / final_answer / changed_files が期待通りに入ること
