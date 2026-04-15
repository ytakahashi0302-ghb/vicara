# EPIC50 Handoff

## Epic 50 で完了したこと

- Claude CLI のストリーミング表示問題を解消し、Dev ターミナル上でエージェントの思考プロセス（thinking）をリアルタイムに追えるようになった。
- Windows 向けには npm shim 解決を追加し、Claude CLI の起動経路を Gemini / Codex と同様の考え方で安定化した。
- Claude の `stream-json` をそのまま見せるのではなく、TerminalDock 側で thinking 中心の可読表示に整形する UX まで整えた。
- PO 実機検証により、Claude の thinking 表示が問題なく動作し、Gemini CLI 側にも回帰がないことを確認済み。

## レトロスペクティブ向けの新しい基盤

- 今回の Epic で、レトロスペクティブに向けた実行ログ蓄積基盤を追加した。
- DB には次の 2 テーブルを追加済み。
  - `agent_retro_runs`
  - `agent_retro_tool_events`
- 保存対象は raw ログ全文ではなく、レトロで必要な最小限の構造化データに絞っている。
  - `reasoning_log`
  - `final_answer`
  - `changed_files_json`
  - `tool events`
  - 実行メタ情報
- Claude は `stream-json` から thinking / 回答 / tool use を構造化抽出して保存する。
- Gemini は現行ログから best-effort で reasoning / final answer を保存する。
- Codex は `--output-last-message` を活用して final answer を保存する。

## 次の Epic 51 への文脈

- Epic 49 で構築した「ふせん」データ基盤と、今回 Epic 50 で構築した `agent_retro_runs` / `agent_retro_tool_events` を組み合わせることで、次フェーズでは「ユーザーの気づき」と「エージェント実行の痕跡」を同時に扱える状態になった。
- 次 Epic では、この蓄積済みデータを材料にして、スクラムマスター（SM）によるレトロスペクティブの自動合成・分析機能へ進むのが自然。
- 特に有望な方向性:
  - ふせん内容と agent run の reasoning / final answer / changed files を突き合わせる
  - 実行ログから「詰まり」「再試行」「無駄な往復」を検出する
  - Keep / Problem / Try の草案を SM が自動提案する

## 次 Epic で意識してほしいこと

- ログ保存は「分析のための構造化データ」が目的であり、生の巨大ログを増やすことが目的ではない。
- そのため、次 Epic でも raw 全保存に寄せるより、レトロで本当に意味のある要約・分類・抽出へ寄せる方がよい。
- Epic 49 から続く UX 方針として、入力体験を重くしないこと、既存フローを邪魔しないことは引き続き最優先。

## 主要な参照ファイル

- `src-tauri/src/claude_runner.rs`
- `src-tauri/src/agent_retro.rs`
- `src-tauri/src/db.rs`
- `src-tauri/src/lib.rs`
- `src/components/terminal/TerminalDock.tsx`
- `src-tauri/migrations/20_agent_retro_logs.sql`
- `docs/50_claude_cli_streaming/task.md`
- `docs/50_claude_cli_streaming/walkthrough.md`
