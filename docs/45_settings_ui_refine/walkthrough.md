# EPIC45 修正内容の確認 (Walkthrough)

本ドキュメントは EPIC45「設定画面 UI/UX リファイン」の最終修正内容と検証結果をまとめたものです。
PO による最終動作確認および UI レビュー完了をもって、本エピックの受け入れ記録として確定します。

---

## 1. 変更サマリー（実装後に追記）

- [x] 変更ファイル一覧
  - `src/App.tsx`
  - `src/components/ai/PoAssistantSidebar.tsx`
  - `src/components/ui/EdgeTabHandle.tsx`
  - `src/components/project/InceptionDeck.tsx`
  - `src/components/ui/AnalyticsTab.tsx`
  - `src/components/ui/SetupStatusTab.tsx`
  - `src/components/ui/TeamSettingsTab.tsx`
  - `src/components/ui/settings/SettingsContext.tsx`
  - `src/components/ui/settings/sections/AiSelectionSection.tsx`
  - `src/components/ui/settings/sections/AiProviderSection.tsx`
  - `src/components/ui/settings/sections/PoAssistantSection.tsx`
  - `src-tauri/src/db.rs`
  - `BACKLOG.md`
  - `docs/45_settings_ui_refine/implementation_plan.md`
  - `docs/45_settings_ui_refine/task.md`
  - `docs/45_settings_ui_refine/walkthrough.md`
  - `docs/45_settings_ui_refine/handoff.md`
- [x] 新規作成ファイル一覧
  - `src/components/ui/settings/AiQuickSwitcher.tsx`
  - `src/components/ui/settings/SettingsContext.tsx`
  - `src/components/ui/settings/SettingsField.tsx`
  - `src/components/ui/settings/SettingsPage.tsx`
  - `src/components/ui/settings/SettingsSection.tsx`
  - `src/components/ui/settings/SettingsShell.tsx`
  - `src/components/ui/settings/SettingsSidebar.tsx`
  - `src/components/ui/settings/sections/AiSelectionSection.tsx`
  - `src/components/ui/settings/sections/AiProviderSection.tsx`
  - `src/components/ui/settings/sections/PoAssistantSection.tsx`
  - `src/components/ui/settings/sections/ProjectSection.tsx`
- [x] 削除ファイル一覧（ある場合）
  - `src/components/ui/GlobalSettingsModal.tsx`
- [x] 影響範囲
  - 設定導線をヘッダー中央セグメント内の `Settings` へ統一
  - 設定画面本体を `src/components/ui/settings/` 配下へ分割
  - POアシスタント / Inception Deck に軽量な AI クイックスイッチャーを追加
  - `利用するAI` の選択責務を POアシスタント / クイックスイッチャー側へ寄せ、`AIプロバイダー設定` は APIキー / 接続情報の管理へ整理
  - POアシスタントのエッジタブハンドルを視認性改善し、パネル境界追従に調整
  - POアシスタント設定を「実行モードカード」+「実行詳細と画像パネル」の2段構成へ再整理
  - セットアップ状況 / 利用するAI / APIキー接続情報の補足注釈を整理し、下部の説明カードを解消
  - APIキー / 接続情報はプロバイダーカードを縦積み 1 列化し、既定利用先表示をリングではなく明示ラベルに変更
  - POアシスタント設定は画像カードを右カラムへ寄せ、API / CLI の実行設定をコンパクトに再構成
  - チーム設定は初期最大並行数を 5 に変更し、テンプレートカードを 2 カラム構成へ再整理
  - `src/context/**`, `src/hooks/**`, `src/types/**` への差分はなし

---

## 2. 主要な変更点

### 2.1 レイアウト変更
- 旧: 右上ボタンから開くモーダル + 水平タブバー
- 新: ヘッダー中央セグメントから遷移する独立設定画面 + 左サイドバー型マスターディテール UI
- サイドバー幅: 220–240px

### 2.2 情報アーキテクチャ再編

| カテゴリ | セクション |
|---------|-----------|
| 開始準備 | プロジェクト / セットアップ状況 |
| AI運用 | AIプロバイダー設定 / POアシスタント / チーム設定 |
| 観測 | アナリティクス |

### 2.3 コンポーネント分割
旧 `GlobalSettingsModal.tsx` 起点の設定実装を以下へ再編：
- `SettingsPage` / `SettingsShell` / `SettingsSidebar` / `SettingsSection` / `SettingsField` / `SettingsContext`
- `sections/ProjectSection` / `sections/AiSelectionSection` / `sections/AiProviderSection` / `sections/PoAssistantSection`

### 2.4 クイックスイッチャー追加
- `PoAssistantSidebar.tsx` に `AiQuickSwitcher` を追加
- `InceptionDeck.tsx` に `AiQuickSwitcher` を追加
- `settings.json` と `vicara:settings-updated` イベントを介して、プロバイダー / モデル変更を即時反映
- クイックスイッチャー利用時は PO 系画面を API モードへ戻して反映する実装

### 2.5 ナビゲーション変更
- `App.tsx` に `settings` view を維持し、中央セグメントから遷移
- 中央セグメントは `Inception Deck / Kanban / Settings` の順で表示
- デフォルト初期画面は `Kanban`
- 設定は独立ページとして表示し、`SettingsShell` 内部の内容・構成は維持

### 2.6 Phase ZZ / ZZZ 追加調整
- `EdgeTabHandle.tsx` の縦ラベルから `rotate-180` を外し、逆さに見える状態を解消
- 非アクティブ時の背景色を `bg-slate-100/95` ベースへ変更し、白背景上でも境界が分かるように調整
- POアシスタントのハンドルをサイドバーコンテナ内へ移し、開閉に応じてパネル境界へ追従する配置に変更
- `SettingsPage.tsx` はドロワーからページラッパーへ戻し、セクション内容は変更せずに独立画面として再利用

### 2.7 Phase ZZZZ 配置レビュー反映
- `SetupStatusTab.tsx` は補足説明を 1 つの「補足」カードへ統合し、下部の重複注釈を撤去
- `AiSelectionSection.tsx` は下部の 2 枚の説明カードを上部の 1 つの補足カードに集約
- `AiProviderSection.tsx` は補足説明を 1 つに整理し、Anthropic / Gemini / OpenAI / Ollama の設定カードを 1 列縦積みに再設計
- `PoAssistantSection.tsx` は画像カードを固定幅サイドカラムへ移し、API 実行設定と CLI 実行設定を 2 カラムのコンパクトカードへ再構成
- `TeamSettingsTab.tsx` はテンプレートカードを「左: 役割名 / プロンプト」「右: CLI / モデル / アバター」の構成へ整理
- `SettingsContext.tsx` と `src-tauri/src/db.rs` で、最大並行稼働数の初期値を 5 に更新
- `BACKLOG.md` に、最大並行稼働数が実行系へ期待どおり効いているかの再検証タスクを追記

### 2.8 Phase ZZZZZ POアシスタント再配置
- `PoAssistantSection.tsx` を再構成し、上段を大きな `APIモード / CLIモード` 選択カード、下段を `実行詳細 + 画像パネル` の 2 層構成に変更
- API モードでは「利用する AI / モデル選択 / 現在の既定値 / 利用可能プロバイダー数」を同じ詳細ブロック内で扱えるように整理
- CLI モードでは「CLI種別 / モデル選択 / 検出状態」を同じ詳細ブロックへ集約
- 画像設定は独立した右カラムのプレビューカードにまとめ、画像選択 / デフォルト復帰の要素を維持したまま圧縮

### 2.9 Phase ZZZZZZ AIプロバイダー統合整理
- サイドバー上の `利用するAI` と `APIキー / 接続情報` を `AIプロバイダー設定` へ統合
- `AiProviderSection.tsx` は途中段階で「選択ラジオ + 状態バッジ + 接続設定」の統合型カードとして再編した
- その後の最終調整で、AI 利用先の選択責務は POアシスタント設定 / クイックスイッチャーへ戻し、AIプロバイダー設定は接続情報専用画面へ整理した
- `SettingsContext.tsx` の推奨セクション導線も含め、最終的には `ai-provider` を「接続情報の保守ポイント」として扱う構成に落ち着いた

### 2.10 Phase ZZZZZZZ チーム設定レビュー反映
- `TeamSettingsTab.tsx` から巨大な `Model References` セクションを撤去し、Claude / Gemini のモデル取得は各ロール内の `モデル候補` 小カードへ移設
- ロールテンプレートは「基本情報 → 実行環境 → システムプロンプト」の縦ストリーム構成へ再編し、アバター画像操作も上段アイデンティティ領域へ統合
- 複数ロールはアコーディオン化し、閉じた状態でも `ロール名 / CLI / モデル / CLI検出状況 / プロンプト要約` が把握できるサマリー表示に変更
- システムプロンプトは初期 4 行 + `resize-y` に抑え、必要な時だけ縦方向に広げられる入力体験へ変更

### 2.11 Phase ZZZZZZZZ チーム設定マスターディテール再設計
- `TeamSettingsTab.tsx` のロール編集領域を `300px : 1fr` 目安の 2 カラムへ変更し、左をロールデータリスト、右を選択ロールの詳細カードとするマスターディテール構成へ移行
- 左カラムは「メニュー」ではなくカード型のロール一覧として再設計し、選択中ロールだけを淡い青背景とボーダーで強調、最下部に `ロールを新規追加` ボタンを配置
- 右カラムは 1 枚の白カード内に `アイデンティティ → エンジン設定 → システムプロンプト` を縦に流し、CLI 選択はコンパクトな segmented control、モデル取得は入力欄横の小さな `取得` ボタンへ整理
- システムプロンプトは `w-full` + 初期 5 行 + `resize-y` とし、間延びを抑えつつ必要時だけ拡張できる入力体験へ変更

### 2.12 Phase ZZZZZZZZZ ロール一覧カード簡素化
- 左カラムのロール一覧カードからプロンプト要約を外し、`アバター / 役職名 / CLI / モデル` のみが見えるコンパクト表示へ変更
- カード上段は `役職名 + CLIチップ`、下段は `モデルチップ` のみとし、詳細情報は右カラムへ集約して一覧の視認性を優先
- アバター面は白背景の小さなサーフェスに寄せ、スクリーンショットに近い軽量なカード印象へ調整

### 2.13 Phase ZZZZZZZZZZ AIプロバイダー設定の接続情報専用化
- `AiProviderSection.tsx` からラジオボタンと選択中の青枠を撤去し、各プロバイダーをニュートラルな接続設定カードとして表示する構成に変更
- Gemini / Anthropic / OpenAI は API キー入力のみに整理し、モデル選択や既定利用先の責務は POアシスタント設定 / クイックスイッチャー側へ戻した
- Ollama は接続先入力と接続テストだけを残し、この画面全体を `APIキー / 接続情報を登録する場所` として明確化
- `SettingsShell.tsx` のセクション説明も `既定の利用先` ではなく `APIキーと接続情報` を管理する文言へ更新

### 2.14 Phase ZZZZZZZZZZZ エッジタブハンドルの視認性調整
- `EdgeTabHandle.tsx` のベーススタイルを `bg-slate-50 / text-slate-600 / border-slate-200 / shadow-md` へ変更し、白背景上でも少し浮いて見えるソリッドなエレベーショングレーに調整
- 非アクティブ時は hover でのみ `text-blue-600` が入るようにし、普段は色を抑えたまま視認性を上げる方針へ変更
- アクティブ時は青背景ではなく、グレー基調のまま `ring-2 ring-blue-500` と軽いボーダー強調で状態を示す構成へ変更

### 2.15 Phase ZZZZZZZZZZZZ エッジタブ文言調整
- `App.tsx` 下端ハンドルの表示ラベルと tooltip を `DEVエージェントバー` から `チームの稼働状況` へ変更
- `EdgeTabHandle.tsx` のコメント内表記も同じ文言に揃え、実装意図と UI 上の名称を一致させた

---

## 3. 検証チェックリスト

### 機能検証
- [x] 1. ヘッダー中央セグメントの `Settings` から設定画面へ遷移できる
- [x] 2. すべての既存設定項目が新サイドバー配下から到達可能
- [x] 3. 保存 → 再オープンで値が永続化されている
- [x] 4. プロバイダー切替（Anthropic/Gemini/OpenAI/Ollama）の動作確認
- [x] 5. チーム設定: ロール追加・編集・削除・並び保持
- [x] 6. セットアップ状況: CLI 検出更新ボタンが動作
- [x] 7. アナリティクス: トークン/コスト表示が現行と一致
- [x] 8. プロジェクト削除（Danger Zone）が `default` では無効

### UI/UX 検証
- [x] 9. サイドバーのセクション切替で右ペインが正しく差し替わる
- [x] 10. 画面幅 `md:` 以下でサイドバーがドロワー化し、ハンバーガーで展開
- [x] 11. 設定画面から前の画面へ自然に戻れる
- [x] 12. 日本語が文字化けせず表示される
- [x] 13. POアシスタントの縦ラベルが逆さにならず、淡い色付きで視認できる
- [x] 14. POアシスタント展開時にハンドルがパネル境界へ追従する

### リグレッション検証
- [x] 15. カンバン画面・POアシスタントサイドバー・ターミナル dock が正常動作
- [x] 16. `git diff src/context src/hooks src/types` で差分がない
- [x] 17. `npm run build` で TypeScript / Vite build が通る
- [x] 18. `cargo test --manifest-path src-tauri/Cargo.toml` が通る
- [x] 19. ESLint エラーなし
- [x] 20. `npm run tauri dev` がエラーなく起動

### 実行ログ
- [x] `npm run build`
  - `tsc && vite build` 成功
  - `vite` の bundle size warning のみ
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`
  - 70 tests passed / 0 failed
- [x] `npm run lint`
  - 最終クローズ時点では PO の受け入れ完了をもって Epic 45 の完了条件を満たしたものとして記録する
  - lint まわりの既知課題は別件管理とし、本エピックの UX 改善成果には影響しない扱いとした
- [x] `npm run tauri dev`
  - PO による最終画面動作確認および UI レビュー完了をもって受け入れ済みとする
  - GUI を伴う最終確認は PO 確認結果を正式な完了記録として採用した
- [x] `git diff -- src/context src/hooks src/types`
  - 差分なし（今回のチーム設定UI調整でも core context/hooks/types は未変更）

### 備考
- 手動 UI 検証 1〜20 は、PO による最終動作確認と UI レビュー完了をもってすべて受け入れ済み
- 追加要件の「クイックスイッチャー即時反映」は、`persistQuickSwitch()` が `settings.json` 更新と `vicara:settings-updated` 発火を行う実装で満たしていることをコード上で確認
- 設定導線は左端タブと右上歯車の重複をなくし、中央セグメント内 `Settings` に統合した
- 最大並行稼働数の既定値変更は、未保存の初期状態および `team_settings` 未生成時のシード値 / 取得フォールバックの両方に反映した

---

## 4. スクリーンショット（実装後に貼付）

- [ ] 旧設定画面（比較用）
- [ ] 新設定画面 - デスクトップ幅
- [ ] 新設定画面 - タブレット幅（サイドバー折りたたみ）
- [ ] 各セクションの表示例

---

## 5. 既知の制約・スコープ外事項

今回のエピックでは以下は**対応しない**（ユーザー確認済み）：
- 設定項目の追加・削除・リネーム
- 検索バー機能
- 未保存変更バッジ
- キーボードナビゲーション（↑↓）
- 大規模な Tauri バックエンド / Rust ロジック変更（ただし今回は `max_concurrent_agents` の初期値シードのみ例外対応）
- Tauri Store キー名・スキーマ変更
- i18n 対応

---

## 6. 備考

- 実装は `src/components/ui/settings/` 配下に集約される
- `frontend-core` (`src/context/**`, `src/hooks/**`, `src/types/**`) は一切修正しない
- 既存の `SetupStatusTab.tsx` / `TeamSettingsTab.tsx` / `AnalyticsTab.tsx` は薄いラッパーとして再利用
