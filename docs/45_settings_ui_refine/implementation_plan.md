# EPIC45 実装計画: 設定画面 UI/UX リファイン

## 1. 背景と目的

現行の `src/components/ui/GlobalSettingsModal.tsx` は **1536 行**の単一コンポーネントで、以下 5 タブを水平タブバーで切り替えている：

1. セットアップ状況
2. POアシスタント設定（約1150行 — 最大）
3. チーム設定
4. アナリティクス
5. プロジェクト設定

Epic39〜43 で設定項目が段階的に追加された結果、以下の課題が顕在化している：

- 水平タブが窮屈で項目数に対してスケールしない
- POアシスタントタブ内でヘッダー・セクション見出しが散在
- 単一コンポーネントに 40+ の state が集中し、保守性が低い
- ユーザーが設定項目の位置を記憶しづらい

本エピックでは **独立した設定画面 + 左サイドバー型マスターディテール UI** への再構築によりこれらを解消する。**機能追加ではなく UI/UX 再設計**が主目的。

---

## 2. 設計方針

### 2.1 レイアウト

設定は右上ボタンで開くモーダルではなく、アプリ内の独立画面として表示する。画面本体を左右 2 ペインに分割：

```
┌──────────────────────────────────────────────────────────┐
│ ☰ Vicara                                        履歴 ... │
├──────────────────────────────────────────────────────────┤
│ 設定画面                                                    │
├──────────────┬───────────────────────────────────────────┤
│  一般         │   ◆ セクションタイトル           │
│  ├ プロジェクト│   一行説明                      │
│  ├ セットアップ│   ─────────────────              │
│              │                                  │
│  AI 運用      │   [SettingsField]               │
│  ├ 利用するAI │   [SettingsField]               │
│  ├ APIキー    │   [SettingsField]               │
│  ├ POアシスト │   [SettingsField]               │
│  ├ チーム設定 │   [SettingsField]               │
│              │                                  │
│  観測         │   [SettingsField]               │
│  ├ アナリティクス│                               │
├──────────────┴──────────────────────────────────┤
│              [ 前の画面へ戻る ] [ 保存 ]          │
└─────────────────────────────────────────────────┘
```

- アプリヘッダー左上にハンバーガーメニューを設置し、`Kanban / Inception Deck / 設定` を切替
- 設定画面はメインコンテンツ領域いっぱいに表示
- サイドバー: 固定幅 220–240px、縦スクロール
- 右ペイン: `flex-1`、独立スクロール
- フッター: 右ペイン下部に固定

### 2.2 情報アーキテクチャ

**3 カテゴリ / 7 セクション** に再編：

| カテゴリ | セクション | 現行タブからの移行 |
|---------|-----------|------------------|
| 開始準備 | プロジェクト | プロジェクト設定タブ |
| 開始準備 | セットアップ状況 | セットアップ状況タブ |
| AI運用 | 利用するAI | 既定の AI プロバイダー選択を独立 |
| AI運用 | APIキー / 接続情報 | Provider/APIキー/Endpoint/モデル |
| AI運用 | POアシスタント | Visual/Execution Mode |
| AI運用 | チーム設定 | チーム設定タブ |
| 観測 | アナリティクス | アナリティクスタブ |

**ポイント**:
- `どれを使うか` と `APIキー設定` を分離するため、`利用するAI` と `APIキー / 接続情報` を別セクション化
- 並び順は「プロジェクト準備 → 接続確認 → 利用AI決定 → 補助設定 → 観測」のユーザーフローに揃える

### 2.3 コンポーネント構成

```
src/components/ui/settings/
├── SettingsPage.tsx           # 独立設定画面のエントリ
├── SettingsShell.tsx          # マスターディテール全体レイアウト
├── SettingsSidebar.tsx        # 左ナビ（カテゴリ＆セクション）
├── SettingsSection.tsx        # 右ペイン共通ラッパー（タイトル/説明）
├── SettingsField.tsx          # ラベル/説明/入力の統一フィールド
├── SettingsContext.tsx        # 設定state/save/dirty管理
└── sections/
    ├── ProjectSection.tsx
    ├── AiSelectionSection.tsx    # どれを使うか
    ├── AiProviderSection.tsx     # APIキー/接続情報
    └── PoAssistantSection.tsx    # Visual/Execution Mode
```

既存ファイルの扱い：
- `GlobalSettingsModal.tsx` → 削除し、`App.tsx` から `settings` view で `SettingsPage` を表示
- `TeamSettingsTab.tsx` / `SetupStatusTab.tsx` / `AnalyticsTab.tsx` → 中身は流用し `SettingsSection` 内にマウント

### 2.4 視覚階層

散在しているヘッダーを 3 階層に統一：

1. **セクションタイトル** (`text-xl font-semibold` + 一行説明)
2. **グループ見出し** (`text-sm font-semibold uppercase tracking-wide text-slate-500` + 区切り線)
3. **フィールドラベル** (`SettingsField` で `label + description + control` を統一配置)

既存 Tailwind トークン（`border-slate-200`, `rounded-xl`, `bg-white/90`）を継続使用。

### 2.5 状態管理

- Tauri Store (`settings.json`) スキーマ・キー名は**変更しない**
- 既存フック (`useCliDetection`, `usePoAssistantAvatarImage`, `useLlmUsageSummary`) をそのまま流用
- 巨大 state を `SettingsContext.tsx`（`src/components/ui/settings/` 配下）に集約
  - CLAUDE.md のルールに従い `src/context/**` は修正しない

### 2.6 レスポンシブ

- `md:` ブレークポイント以下でサイドバーを折りたたみ、ハンバーガーボタンで展開するドロワー形式に切替。
- それ以上の幅では常時サイドバー表示。

**今回スコープ外**（ユーザー確認済み）: 検索バー / 未保存変更バッジ / キーボードナビ

---

## 3. 修正対象ファイル

### 新規作成
- `src/components/ui/settings/SettingsShell.tsx`
- `src/components/ui/settings/SettingsPage.tsx`
- `src/components/ui/settings/SettingsSidebar.tsx`
- `src/components/ui/settings/SettingsSection.tsx`
- `src/components/ui/settings/SettingsField.tsx`
- `src/components/ui/settings/SettingsContext.tsx`
- `src/components/ui/settings/sections/ProjectSection.tsx`
- `src/components/ui/settings/sections/AiSelectionSection.tsx`
- `src/components/ui/settings/sections/PoAssistantSection.tsx`
- `src/components/ui/settings/sections/AiProviderSection.tsx`

### 修正
- `src/App.tsx` — `settings` view とハンバーガーメニューを追加
- `src/components/ui/SetupStatusTab.tsx` — `SettingsSection` 内での利用に調整
- `src/components/ui/TeamSettingsTab.tsx` — 同上
- `src/components/ui/AnalyticsTab.tsx` — 同上
- `src/components/ai/PoAssistantSidebar.tsx` — クイックスイッチャー表示
- `src/components/project/InceptionDeck.tsx` — クイックスイッチャー表示

### 参照のみ（修正禁止）
- `src/components/ui/Modal.tsx` / `Button.tsx` / `Input.tsx` / `Textarea.tsx` / `Card.tsx` / `AvatarImageField.tsx` / `WarningBanner.tsx`
- `src/context/WorkspaceContext.tsx`
- `src/hooks/useCliDetection.ts` / `useLlmUsageSummary.ts` / `usePoAssistantAvatarImage.ts`
- `src/App.tsx`（設定モーダル呼び出し L227 付近）

---

## 4. 再利用する既存資産

- **UIプリミティブ**: `Button`, `Input`, `Textarea`, `Card`, `Modal`, `AvatarImageField`, `WarningBanner`
- **スタイルトークン**: `border-slate-200`, `bg-white/90`, `rounded-xl`, `shadow-*`
- **フック**: `useCliDetection`, `useLlmUsageSummary`, `usePoAssistantAvatarImage`
- **永続化**: Tauri Store (`settings.json`)

---

## 5. テスト方針

自動テストが未整備のため、**手動検証チェックリスト**を walkthrough に記録する方針で進める。

### 機能検証
1. 設定モーダルの開閉が正常動作
1. 左上ハンバーガーメニューから `Kanban / Inception Deck / 設定` に遷移できる
2. すべての既存設定項目が新サイドバー配下から到達可能
3. 保存 → 再オープンで値が永続化されている（全カテゴリ）
4. プロバイダー切替（Anthropic/Gemini/OpenAI/Ollama）で APIキー入力・モデル取得ボタンが動作
5. チーム設定: ロール追加・編集・削除・並び保持
6. セットアップ状況: CLI 検出更新ボタン動作
7. アナリティクス: トークン/コスト表示が現行と一致
8. プロジェクト削除（Danger Zone）が `default` プロジェクトで無効

### UI/UX 検証
9. サイドバーのセクション切替で右ペインが正しく差し替わる
10. 画面幅 `md:` 以下でサイドバーがドロワー化し、ハンバーガーで展開
11. 設定画面から前の画面へ自然に戻れる
12. 日本語が文字化けせず表示される

### リグレッション検証
13. カンバン画面・POアシスタントサイドバー・ターミナル dock が影響を受けていない
14. `frontend-core` (`src/context/**`, `src/hooks/**`, `src/types/**`) に差分なし（`git diff` で確認）
15. TypeScript 型エラー・ESLint エラーなし

### 実行手順
```bash
npm run tauri dev
```
起動後、左上ハンバーガーメニューから設定画面へ遷移し、上記 1–15 を順に実施。結果を `walkthrough.md` に記録。

---

## 6. 実装順序

1. **骨格**: `SettingsContext` → `SettingsShell` → `SettingsSidebar` → `SettingsSection` → `SettingsField`
2. **既存タブ移植**: SetupStatus → Analytics → Team（薄いラッパー適用のみで済むもの順）
3. **POアシスタント分割**: `ProjectSection` → `PoAssistantSection` → `AiProviderSection`
4. **統合**: `App.tsx` に `settings` view とハンバーガーナビを追加
5. **レスポンシブ**: ドロワー挙動の追加
6. **検証**: walkthrough.md チェックリスト消化

---

## 7. リスクと対策

| リスク | 対策 |
|-------|------|
| state の移行でセットアップ値が失われる | 最初に `SettingsContext` に現行 state を完全複製し、段階移行 |
| 既存 Tab コンポーネントの props 互換崩れ | 既存 props を維持し、ラッパー側で吸収 |
| Tauri Store キー衝突 | キー名一切変更しない方針を徹底 |
| `frontend-core` への意図せぬ変更 | 実装前後で `git diff src/context src/hooks src/types` を確認 |

---

## 8. v2 リファイン改訂（Phase Z 設計）

### 8.1 改訂の背景
Phase Y までの実装（他 AI エージェントによる）で以下のズレが発生：

| 項目 | 意図 | 実装結果 |
|---|---|---|
| ビュー切替 | Kanban/Deck はヘッダーに残す | ハンバーガー内に集約されてしまった |
| 設定の表示形態 | サイドバー（ドロワー）型 | 全画面 view swap 型 |
| ヘッダー情報量 | 削減したい | Current View 表示 / POボタン / 多数のピルが残存 |
| 視覚デザイン | 落ち着いた配色 | sky/violet/orange/cyan 等の多色カードで乱雑 |
| PO/Dev バー呼び出し | マウス動線最短のエッジタブ | ヘッダー上ボタンからの呼び出しのまま |

Phase Z はこの 5 点を再実装する。

### 8.2 新しいレイアウト全景

```
┌────────────────────────────────────────────────────────────────┐
│ ロゴ  Vicara   [ Kanban | Inception Deck ]   📦Proj 履歴 ¥123 │  ← ヘッダー簡素化
├─┬──────────────────────────────────────────────────────────┬─┤
│⚙│                                                          │P│
│ │                                                          │O│
│設│                                                          │ │
│定│               (メインコンテンツ: Kanban or Deck)           │ア│
│ │                                                          │シ│
│ │                                                          │ス│
│ │                                                          │タ│
│ │                                                          │ン│
│ │                                                          │ト│
├─┴──────────────────────────────────────────────────────────┴─┤
│              [  ▲ DEVエージェントバー  ]                     │  ← 下端タブ
└────────────────────────────────────────────────────────────────┘
```

設定ドロワーを開いた状態（左からオーバーレイ、背後スクリム）：

```
┌────────────────────────────────────────────────────────────────┐
│ ロゴ  Vicara   [ Kanban | Inception Deck ]   📦Proj 履歴 ¥123 │
├──────────────────────┬──────────────────────────────────┬─────┤
│ 設定              × │                                  │  P  │
│ ─────────────────── │                                  │  O  │
│ 開始準備             │                                  │     │
│  プロジェクト         │ (スクリムで暗転・クリックで閉じる) │     │
│  セットアップ         │                                  │     │
│                     │                                  │     │
│ AI 運用              │                                  │     │
│  利用する AI         │                                  │     │
│  API キー           │                                  │     │
│  PO アシスタント      │                                  │     │
│  チーム設定           │                                  │     │
│                     │                                  │     │
│ 観測                 │                                  │     │
│  アナリティクス        │                                  │     │
│              [保存]  │                                  │     │
└──────────────────────┴──────────────────────────────────┴─────┘
```

### 8.3 コンポーネント変更詳細

#### 新規
- `src/components/ui/EdgeTabHandle.tsx`
  - Props: `side: "left" | "right" | "bottom"`, `icon`, `label`, `active`, `onClick`, `badge?`
  - 縦配置（left/right）ではラベルを `writing-mode: vertical-rl` または Tailwind `[writing-mode:vertical-rl]` で縦書き
  - active 時はパネル側に寄り付き、`>` → `<` のようなハンドル形状に変化

#### 修正
- **`src/App.tsx`**
  - `currentView` から `"settings"` を削除、`isSettingsOpen` state に分離
  - ハンバーガーメニュー / Current View 表示 / POボタン / Scaffold常設ボタンを撤去
  - ヘッダー中央に `Kanban / Inception Deck` セグメント追加
  - `LlmUsagePill` を集約数値 + hover tooltip に簡略化
  - メインレイアウトを `flex`: `[設定ドロワー] [メイン] [POパネル]` + 下に `[ターミナル]`
  - 左端・右端・下端に `EdgeTabHandle` を配置
- **`src/components/ui/settings/SettingsPage.tsx`**
  - 「オーバーレイ型ドロワー」コンポーネントに変更（**ユーザー確認済み: オーバーレイ型**）
  - `isOpen` を受け取り、左からスライドインするパネル（幅 520px 固定）
  - 背後にスクリム（`bg-slate-900/40 backdrop-blur-sm`）を敷き、クリックで閉じる
  - メインコンテンツはレイアウトに影響を与えない（`position: fixed` または `absolute` でオーバーレイ）
  - アニメーション: `translate-x` の transition 200ms
  - 内部に `SettingsShell` をそのままマウント
- **`src/components/ui/settings/SettingsShell.tsx`**
  - ドロワー幅（520px）前提にナビ幅を 180px に調整
  - ヘッダーに「設定」タイトル + × ボタン
- **`src/components/ui/settings/sections/AiProviderSection.tsx`**
  - プロバイダー色分けを廃止、白カード + モノクロアイコン + `ring-2 ring-blue-500` 選択状態に統一
- **`src/components/ui/settings/sections/PoAssistantSection.tsx`**
  - グラデーション背景撤去
  - cyan/blue アクセント削除、`ring-2 ring-blue-500` のみ
- **`src/components/ui/settings/sections/ProjectSection.tsx` / `AiSelectionSection.tsx`**
  - 同スタイルルールに揃える
- **`src/components/ui/SetupStatusTab.tsx` / `TeamSettingsTab.tsx` / `AnalyticsTab.tsx`**
  - 色バッジ削減、白カード + モノクロアイコンに統一
- **`src/components/ai/PoAssistantSidebar.tsx`**
  - 内部ヘッダーから「閉じる」ボタンを削除可能（エッジタブに統合）、もしくは併存
- **`src/components/terminal/TerminalDock.tsx`**
  - 同様にヘッダー開閉ボタンをエッジタブに移譲
- **`src/components/ui/settings/AiQuickSwitcher.tsx`**
  - スタイルのみ新ルールに準拠（多色グラデ禁止）

### 8.4 スタイルガイド（コードコメントとして `SettingsShell.tsx` 先頭に記載）

```
/**
 * Settings UI Style Guide (EPIC45 v2)
 * - Primary: blue-600 only
 * - Card bg: bg-white + border-slate-200
 * - Selection state: ring-2 ring-blue-500
 * - Radius: rounded-xl (no rounded-2xl)
 * - Padding: card=p-5, inner=p-4, section gap=space-y-6
 * - Semantic colors: red=danger, amber=warning, emerald=success ONLY
 * - NO provider-specific brand colors (sky/violet/orange/cyan forbidden)
 * - NO gradient backgrounds
 * - Typography: title text-lg font-semibold text-slate-900 / desc text-sm text-slate-500
 */
```

### 8.5 実装順序（Phase Z）

1. `EdgeTabHandle` コンポーネント作成 + Storybook 的に単独確認
2. `App.tsx` ヘッダー簡素化 + セグメント追加 + `isSettingsOpen` state 分離
3. `SettingsPage` をプッシュ型ドロワー化、`App.tsx` レイアウトに組込
4. 左端・右端・下端に `EdgeTabHandle` を配置し、既存トグルを置換
5. スタイル統一パス: `AiProviderSection` → `PoAssistantSection` → 残りセクション → `*Tab.tsx`
6. `AiQuickSwitcher` スタイル調整
7. 動作検証（Phase Z-6 チェックリスト）

### 8.6 テスト方針（v2 追加分）

- 既存の Phase 4 チェックリストに加え、以下を確認：
  - メインコンテンツがドロワー展開に合わせて正しくリサイズされる
  - エッジタブのホバー/アクティブ状態が視認できる
  - ドロワー開閉のアニメーションが 200ms 程度でカクつかない
  - `Esc` キーでドロワーを閉じられる
  - PO パネル・設定ドロワー・ターミナル dock を全部同時に開いても main コンテンツが最小幅を保つ
  - `git diff src/context src/hooks src/types` が空

### 8.7 スコープ外（Phase Z でも扱わない）

- 設定項目の追加・削除・文言変更
- Tauri Store キー・スキーマ変更
- カンバン / Inception Deck 本体のレイアウト変更（ヘッダー・エッジタブの追加/削除のみ）
- バックエンド Rust コード変更
- i18n 対応

### 8.8 追加修正（2026-04-11）

別 AI エージェントによる Phase Z 後の微調整として、以下を追加で実施する。

- PO アシスタントのエッジタブハンドルは、縦書きラベルが逆さに見えない向きへ修正する
- エッジタブハンドルは白ベースのままでは埋もれるため、ニュートラル寄りの淡い色を付けて視認性を上げる
- PO アシスタント展開時は、ハンドルが画面端固定ではなくサイドパネル境界に追従する配置へ変更する
- 設定導線は左端エッジタブを廃止し、ヘッダー右上の歯車ボタンから遷移する独立ページに戻す
- `SettingsShell` 以下の情報設計・セクション構成・フォーム内容は変更しない

#### 追加テスト観点

1. PO アシスタントの縦ラベルが逆さに見えない
2. ハンドル非アクティブ時でも背景に埋もれず識別できる
3. PO アシスタントを開閉するとハンドルがパネル境界に追従する
4. 右上歯車から設定ページへ遷移し、保存後も従来どおり値が保持される
5. 設定ページのセクション内容・並び・保存導線が崩れていない
