# Epic 20 → Epic 21 引き継ぎ書: 次世代エージェント基盤 バックエンド PoC 完了

**作成日:** 2026-04-01
**作成者:** Epic 20 実装エージェント
**宛先:** Epic 21 担当エージェント（React フロントエンド実装）

---

## 1. バックエンド PoC 完了サマリー

Epic 20 では、MicroScrum AI の次世代 AI エージェント基盤として以下の 2 トラックを完全実装した。フロントエンド（React/TypeScript）への変更は一切行っていない。

### Track A: Rig ベース AI 抽象化層

| 実装内容 | ファイル |
|---------|---------|
| `AiProvider` enum（Anthropic / Gemini の型安全な切り替え） | `src-tauri/src/rig_provider.rs` |
| `resolve_provider_and_key()` — Tauri Store からプロバイダ・APIキーを解決 | 同上 |
| `chat_with_history()` — 会話履歴付き統一 LLM 呼び出し API | 同上 |
| 全 4 AI コマンド（generate_tasks_from_story, refine_idea, chat_inception, chat_with_team_leader）を Rig 経由に移行 | `src-tauri/src/ai.rs` |
| API タイムアウト 60 秒（Anthropic / Gemini 両プロバイダ） | `src-tauri/src/rig_provider.rs` |

**意義:** 以前は reqwest による生 HTTP リクエストだったが、Rig により以下が可能になった。
- プロバイダ（Anthropic/Gemini）の設定ベース切り替え
- 会話履歴の型安全な管理
- 将来のツール統合・エージェントロジック拡張の土台

### Track B: PTY ベースコマンド実行基盤

| 実装内容 | ファイル |
|---------|---------|
| `PtyManager` — セッション管理（Windows/Unix プラットフォーム分岐） | `src-tauri/src/pty_manager.rs` |
| `ExecutionResult { exit_code, stdout, stderr }` — 構造化された実行結果 | 同上 |
| Tauri コマンド 3 種（pty_spawn, pty_execute, pty_kill） | `src-tauri/src/pty_commands.rs` |
| PTY セッション自動クリーンアップ（5分間隔、30分アイドルで kill） | `src-tauri/src/lib.rs` |
| ログ基盤（log + env_logger、`RUST_LOG` 環境変数で制御） | `src-tauri/src/main.rs` / 各モジュール |

**意義:** AI Dev エージェントが実際にローカル環境でコマンドを実行する基盤が完成した。フロントエンドから `invoke()` を呼ぶだけでコマンド実行・出力取得が可能。

---

## 2. フロントエンドから呼び出せる Tauri API リファレンス

### PTY コマンド（新規）

#### `pty_spawn` — シェルセッション起動

```typescript
const sessionId: string = await invoke('pty_spawn', {
  cwd: 'C:\\Users\\green\\project'  // 作業ディレクトリ（絶対パス）
});
// 戻り値: UUID 文字列 (例: "550e8400-e29b-41d4-a716-446655440000")
// エラー時: string をスロー
```

#### `pty_execute` — コマンド実行

```typescript
interface ExecutionResult {
  exit_code: number;   // 終了コード。0 = 成功、非0 = 失敗
  stdout: string;      // 標準出力
  stderr: string;      // 標準エラー出力（Windows のみ分離。Unix は stdout に混在）
}

const result: ExecutionResult = await invoke('pty_execute', {
  session_id: sessionId,
  command: 'npm run build'
});

if (result.exit_code === 0) {
  console.log('成功:', result.stdout);
} else {
  console.error('失敗 (exit_code:', result.exit_code, '):', result.stderr || result.stdout);
}
```

**重要な制約:**
- タイムアウト: 30 秒（長時間コマンドは現時点では対応外）
- Windows: `cmd.exe /C <command>` で実行。各コマンドは独立プロセス（環境変数の引き継ぎなし）
- Unix: PTY ベース。stdout/stderr が統合されるため stderr は常に空文字列
- `cd` コマンド: セッションの CWD として追跡される（次回 execute_command に反映）

#### `pty_kill` — セッション終了

```typescript
await invoke('pty_kill', { session_id: sessionId });
// 戻り値: void
// エラー時: string をスロー（セッション未発見など）
```

**セッション管理の注意点:**
- セッションは 30分間操作がないと自動 kill される（バックグラウンドタスクが 5分間隔で掃除）
- フロントエンドは画面クローズ時に明示的に `pty_kill` を呼ぶことを推奨

---

### 既存 AI コマンド（変更なし・引き続き利用可能）

これらのコマンドは Epic 20 で Rig 経由に移行されたが、フロントエンドの `invoke()` インターフェースは完全に維持されている。

| コマンド | 用途 |
|---------|-----|
| `generate_tasks_from_story` | ストーリーからタスクを AI 生成 |
| `refine_idea` | アイデアを AI でリファイン |
| `chat_inception` | インセプションデッキ生成チャット |
| `chat_with_team_leader` | AI チームリーダーとの会話 |

AI API タイムアウトは 60 秒に設定済み。

---

## 3. 将来の課題（Future Backlog）

以下は設計方針レビュー（`docs/etc/設計方針.txt`）で提案されたが、PoC フェーズでは**意図的に見送った**項目。UI 実装時にこれらが「現在は未実装」であることを前提に設計すること。

### 優先度: 高（本格稼働前に必要）

| 課題 | 現状の制約 | 将来の解決策 |
|-----|-----------|------------|
| **Shell Injection 対策** | `command: string` を `cmd.exe /C` に直接渡している | `CommandSpec { program, args, env }` 構造体化で構造的に防止 |
| **リアルタイム出力ストリーミング** | `pty_execute` は完了まで待機してから返す（最大 30 秒ブロック） | `mpsc::channel` + Tauri イベントで行単位リアルタイム配信 |
| **長時間コマンドのキャンセル** | タイムアウト 30 秒後に Err を返すのみ | OutputEvent ストリームと連動したキャンセル機構 |

### 優先度: 中（スケール時に必要）

| 課題 | 現状の制約 | 将来の解決策 |
|-----|-----------|------------|
| **Unix stderr 分離** | PTY の性質上 stdout/stderr が統合 | tmpfile リダイレクト等で分離 |
| **環境変数の引き継ぎ（Windows）** | Windows では各コマンドが独立プロセスのため `export FOO=bar` が次コマンドに反映されない | `PtySession.env: HashMap<String,String>` + コマンド前に注入 |
| **CommandLog 監査基盤** | 実行履歴が残らない | 実行コマンド・exit_code・時刻を SQLite に記録 |

### 優先度: 低（将来の拡張）

- **CommandExecutor trait**: Windows/Unix/SSH/Docker を共通 trait で抽象化
- **エラー型の構造化**: 現在は `String` エラー。構造化 enum で詳細なエラーハンドリングが可能

---

## 4. 次期 Epic（3ペインUI）への申し送り

### Epic 21 の目的

Epic 20 で完成したバックエンド API を使い、**React フロントエンドに「AI Dev エージェント操作パネル」を実装する**ことが次の目標。

### 推奨 UI アーキテクチャ（参考）

```
┌─────────────────────────────────────────────────────────┐
│  Left Pane: タスク一覧         Center: ターミナル        │
│  （既存 Kanban から流用）      （xterm.js ベース）       │
│                                invoke('pty_execute', …) │
│                               ┌──────────────────────┐ │
│  Right Pane: AI チャット      │ $ npm run build       │ │
│  （Team Leader Sidebar        │ ✓ Build succeeded     │ │
│   を拡張 or 新規コンポーネント）│ $ git commit …       │ │
│                               └──────────────────────┘ │
└─────────────────────────────────────────────────────────┘
```

### 推奨ライブラリ

| 用途 | 推奨 |
|-----|-----|
| ターミナル UI | `xterm.js` + `xterm-addon-fit` |
| Tauri invoke | 既存の `@tauri-apps/api` を流用 |
| ターミナルとの接続 | `invoke('pty_execute', ...)` の結果を xterm に `term.write()` |

### セッションライフサイクル管理（フロントエンド責務）

```typescript
// コンポーネントマウント時
const sessionId = await invoke('pty_spawn', { cwd: project.localPath });

// コマンド実行時
const result = await invoke('pty_execute', { session_id: sessionId, command });
term.write(result.stdout);
if (result.exit_code !== 0) term.write(`\r\n[exit: ${result.exit_code}]\r\n`);

// コンポーネントアンマウント時（必須）
await invoke('pty_kill', { session_id: sessionId });
```

### 既知の注意点

1. **Windows で `cd` コマンドを使う場合**: バックエンドが CWD を追跡しているが、フロントエンド側でも現在 CWD を表示状態として持つことを推奨
2. **stdout のエンコーディング**: Windows (`cmd.exe /C`) の出力は CP932（Shift-JIS）の場合あり。日本語混在環境では `chcp 65001` を先行実行するか、フロントで文字コード変換を検討
3. **セッション自動 kill**: 30分アイドルでバックエンドが自動 kill。フロントでセッション生存確認の仕組みを設けるか、定期的に軽量コマンド（`echo ping`）を送る keepalive 実装を検討

---

## 5. バックエンド最終状態サマリー

```
src-tauri/src/
├── ai.rs           — 4 AI コマンド（Rig 経由に移行済み）
├── db.rs           — 全 CRUD コマンド
├── inception.rs    — インセプションデッキ生成
├── lib.rs          — Tauri Builder / invoke_handler / PTY cleanup タスク
├── main.rs         — env_logger 初期化
├── pty_commands.rs — pty_spawn / pty_execute / pty_kill
├── pty_manager.rs  — PtyManager / ExecutionResult（Windows + Unix 実装）
└── rig_provider.rs — AiProvider / chat_with_history（Anthropic + Gemini）
```

`cargo check` / `cargo clippy` ともにクリーン（既知の軽微な警告 2 件のみ）。

---

*本書は Epic 20 の完了をもって本ドキュメントは確定版となる。以降の変更は Epic 21 の実装ドキュメントに記録すること。*
