# EPIC57 実装計画 — デザインブラッシュアップ

## 背景・目的

アプリの機能は十分に拡充されたが、以下の問題が蓄積している。

- `gray-*` と `slate-*` が同一コンポーネント内で混在
- 角丸（`rounded-md` / `rounded-lg` / `rounded-xl` / `rounded-2xl`）が場所によってバラバラ
- ストーリータイトルなどで `truncate` がなく2行に崩れる箇所がある
- ヘッダー右クラスターに異なる性質の要素（コスト・プロジェクト名・フォルダ・履歴）が混在
- Dev エージェント（ターミナル）の開き方が分かりにくい
- Board のプレビューボタンが大きなカード型で、ヘッダー行のバランスを崩している
- `sky-*` カラーが Board のみで使われ、システム全体と不統一

**方針**: 機能変更なし。Tailwind クラスとコンポーネント構造のみ変更する。

---

## 対象モジュール

| モジュール | 対象ファイル |
|-----------|------------|
| frontend-core | `src/App.tsx`, `src/components/ui/*` |
| frontend-kanban | `src/components/kanban/*`, `src/components/board/*` |
| frontend-ai | `src/components/ai/PoAssistantSidebar.tsx`（バッジ連携のみ） |
| frontend-terminal | `src/components/terminal/TerminalDock.tsx`（バー改修） |

---

## Phase 別実装詳細

### Phase 1: デザイントークン定義
**対象**: `tailwind.config.js`

カスタムテーマを定義し、後続 Phase の基準とする。

```js
// tailwind.config.js
theme: {
  extend: {
    borderRadius: {
      badge: '0.25rem',   // rounded-md相当（バッジ・タグ）
      card:  '0.75rem',   // rounded-xl相当（カード・パネル）
    },
    boxShadow: {
      card:  '0 1px 3px 0 rgb(0 0 0 / 0.07)',
      panel: '0 4px 6px -1px rgb(0 0 0 / 0.07)',
    },
  },
}
```

**統一方針**:
- ニュートラルカラーは `slate-*` に統一（`gray-*` は廃止方向）
- アクセントカラーは `blue-*` に統一（`sky-*` は廃止）
- 角丸: バッジ/タグ → `rounded-md`、カード/パネル → `rounded-xl`、全体ボタン → `rounded-xl`
- シャドウ: カード → `shadow-sm`、フローティングパネル → `shadow-md`

---

### Phase 2: カラー統一
**対象**: `App.tsx`, `TaskCard.tsx`, `StorySwimlane.tsx`, `StatusColumn.tsx`, `BacklogView.tsx`, `Button.tsx`

主な置換パターン：

| 変更前 | 変更後 | 場所 |
|--------|--------|------|
| `bg-gray-100` | `bg-slate-100` | `App.tsx`, `Board.tsx` |
| `bg-gray-50` | `bg-slate-50` | `StorySwimlane.tsx` ヘッダー |
| `border-gray-200` | `border-slate-200` | 各カード |
| `text-gray-500` | `text-slate-500` | 説明文・補助テキスト |
| `text-gray-900` | `text-slate-900` | 見出しテキスト |
| `bg-gray-200` (Button Secondary) | `bg-slate-200` | `Button.tsx` |
| `border-sky-200`, `bg-sky-*`, `text-sky-*` | `border-blue-200`, `bg-blue-*`, `text-blue-*` | `Board.tsx` プレビューボタン |

---

### Phase 3: カードデザイン統一
**対象**: `TaskCard.tsx`, `StorySwimlane.tsx`, `StatusColumn.tsx`, `BacklogView.tsx`

統一後の基本スタイル:

```
カード全般:  bg-white rounded-xl border border-slate-200 shadow-sm
スイムレーン: bg-white rounded-xl border border-slate-200 shadow-sm mb-4
タスクカード: rounded-xl（現在 rounded-md から変更）
列コンテナ:  rounded-xl（現在通り、shadow を shadow-sm に統一）
```

---

### Phase 4: テキストオーバーフロー対応
**対象**: `StorySwimlane.tsx`, `BacklogView.tsx`

```tsx
// StorySwimlane.tsx — ストーリータイトル（現在: 無制限）
<h2 className="truncate text-lg font-semibold text-slate-900" title={story.title}>
  {story.title}
</h2>

// BacklogView.tsx — バックログアイテムタイトル
<span className="line-clamp-2 text-sm font-medium text-slate-900">
  {item.title}
</span>

// ステータスバッジ全般
<span className="... whitespace-nowrap">...</span>
```

---

### Phase 5: ヘッダーリファイン
**対象**: `src/App.tsx`（`AppHeader` 関数 / `AppContent` 関数）、`ScrumDashboard.tsx`、`Board.tsx`

**変更内容**:

1. `LlmUsagePill` をヘッダーから削除し、`Board.tsx` のスプリントボードヘッダー行（タイトル横）に移動
2. 履歴ボタン（`onOpenHistory`）をヘッダーから削除し、`Board.tsx` ヘッダー行に移動
3. ヘッダー右クラスターは `[プロジェクト名▼ | フォルダ⚙]` のみに整理

```tsx
// ヘッダー右クラスター（変更後）
<div className="flex items-center gap-2 rounded-xl border border-slate-200 bg-slate-50/80 px-2 py-1 shadow-sm">
  <ProjectSelector />
  <div className="hidden h-8 w-px bg-slate-200 sm:block" />
  <ProjectSettings />
</div>

// Board.tsx ヘッダー行（変更後）
<div className="mb-6 flex justify-between items-center">
  <div>
    <h1>スプリントボード</h1>
    <p>{formatSprintLabel(activeSprint)}</p>
  </div>
  <div className="flex items-center gap-3">
    <LlmUsagePill projectId={currentProjectId} />   // ← ここに移動
    <button onClick={onOpenHistory}>履歴</button>     // ← ここに移動
    <PreviewButton ... />                             // ← 整理後のボタン
  </div>
</div>
```

`LlmUsagePill` と `onOpenHistory` は props として `Board` に渡す設計に変更。

---

### Phase 6: 動作確認ボタン整理
**対象**: `src/components/kanban/Board.tsx`

**変更内容**:

- `min-h-[56px]` の3行カード型ボタン → 標準の `h-10` ボタン1個に変更
- `sky-*` → `blue-*` 統一
- `rounded-2xl` → `rounded-xl` 統一
- URL サブタイトルをボタン内から除去し `title` 属性（tooltip）へ移動
- 「停止」ボタンの別出現を廃止 → 同じボタンがアイコン+テキスト切替で2状態に

```tsx
// 変更後のボタン（起動前）
<Button variant="secondary" size="md"
  className="rounded-xl border-blue-200 text-blue-700 hover:bg-blue-50"
  title={rootPreviewSubtitle}
>
  <Eye size={15} className="mr-1.5" />
  動作確認
</Button>

// 変更後のボタン（起動中 / 停止）
<Button variant="secondary" size="md"
  className="rounded-xl border-rose-200 text-rose-700 hover:bg-rose-50"
  onClick={handleStopRootPreview}
  title={`停止: ${previewInfo?.url}`}
>
  <Square size={15} className="mr-1.5" />
  停止
</Button>
```

---

### Phase 7: Dev Agent / PO アシスタント 動線改善
**対象**: `src/App.tsx`, `src/components/terminal/TerminalDock.tsx`, `src/components/ui/EdgeTabHandle.tsx`

#### 7-A: ターミナル最小化バーをクリック可能な帯に改修

`TerminalDock.tsx` の最小化状態 UI を改修。現在の34px 最小化バーを全体クリック可能なボタン帯にする。

```tsx
// TerminalDock.tsx — 最小化時の表示（変更後）
{isMinimized && (
  <button
    onClick={onToggleMinimize}
    className="flex h-full w-full items-center gap-2 px-4 text-gray-400 hover:text-gray-200 transition-colors"
  >
    <TerminalSquare size={14} className="shrink-0" />
    <span className="text-xs font-semibold uppercase tracking-[0.12em]">Dev Agent</span>
    <span className="ml-auto text-xs opacity-50">▲ 開く</span>
  </button>
)}
```

#### 7-B: EdgeTabHandle のラベル変更

```tsx
// App.tsx — 下部フロートハンドル
<EdgeTabHandle
  side="bottom"
  label="Dev Agent"           // 変更: チームの稼働状況 → Dev Agent
  icon={TerminalSquare}
  active={!isTerminalMinimized}
  badge={isAgentRunning ? '●' : undefined}   // 稼働中バッジ
  ...
/>

// App.tsx — 右端ハンドル
<EdgeTabHandle
  side="right"
  label="PO"                  // 変更: PO アシスタント / ふせん → PO
  icon={Bot}
  active={isSidebarOpen}
  badge={hasUnreadPoMessage ? '●' : undefined}   // 未読バッジ
  ...
/>
```

#### 7-C: バッジ状態の管理

- `isAgentRunning`: `TerminalDock` が既に持つエージェント稼働状態を `App.tsx` に lift up
- `hasUnreadPoMessage`: `PoAssistantSidebar` の未読フラグを `App.tsx` に lift up
- バッジは静的なドット（`●`）のみ。アニメーションなし

---

### Phase 8: フォーカスリング統一
**対象**: `Button.tsx`, `Input.tsx`, `Textarea.tsx`, `Modal.tsx`

```
統一後: focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2
```

`ring-offset-2` が抜けている箇所に追加。

---

### Phase 9: Badge コンポーネント化
**対象**: `src/components/ui/Badge.tsx`（新規）、`TaskCard.tsx`、`StorySwimlane.tsx`

```tsx
// src/components/ui/Badge.tsx
interface BadgeProps {
  variant: 'priority' | 'status';
  level?: 1 | 2 | 3 | 4 | 5;       // priority 用
  status?: Task['status'];           // status 用
}
```

`TaskCard.tsx` と `StorySwimlane.tsx` に重複実装されている優先度バッジロジックをこのコンポーネントに統合。

---

## 実施順序

```
Phase 1（トークン定義）→ Phase 2（カラー）→ Phase 3（カード）→ Phase 4（テキスト）
→ Phase 5（ヘッダー）→ Phase 6（プレビューボタン）→ Phase 7（動線）
→ Phase 8（フォーカス）→ Phase 9（バッジ）
```

Phase 1〜4 は機械的な置換が中心で独立性が高い。Phase 5〜7 はコンポーネント間の props 受け渡しを伴うため慎重に。

---

## テスト方針

各 Phase 完了後に開発サーバー（`npm run tauri dev`）で以下を目視確認：

| 確認項目 | 対象 Phase |
|---------|-----------|
| カード崩れなし（テキスト truncate 正常動作） | 3, 4 |
| ドラッグ＆ドロップ正常動作 | 3 |
| ヘッダーの要素配置・折り返し動作 | 5 |
| プレビューボタン：起動 → 停止の状態切替 | 6 |
| ターミナルバーのクリックで展開・格納 | 7 |
| EdgeTabHandle のバッジ表示（エージェント稼働時） | 7 |
| サイドバー・ターミナルのリサイズ動作 | 7 |
| スプリント作成・タスク移動・AI chat | 全体回帰 |
| フォーカスリングの表示（Tab キー移動） | 8 |
