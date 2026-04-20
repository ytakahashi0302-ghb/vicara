# EPIC57 修正内容の確認 (Walkthrough)

## 変更の全体像

機能は一切変更せず、スタイリングとコンポーネント構造のみを整理する。

---

## Phase 1: デザイントークン定義

**`tailwind.config.js`**
- `theme.extend` にカスタム `borderRadius`・`boxShadow` を追加
- 以降の Phase で参照する基準値を確定

---

## Phase 2: カラー統一

**`src/App.tsx`**
- `bg-gray-100` → `bg-slate-100`（コンテナ背景）

**`src/components/kanban/TaskCard.tsx`**
- `text-gray-900`, `text-gray-500` 等を `slate-*` 系に統一

**`src/components/kanban/StorySwimlane.tsx`**
- ヘッダー背景 `bg-gray-50` → `bg-slate-50`
- ボーダー `border-gray-200` → `border-slate-200`

**`src/components/kanban/StatusColumn.tsx`**
- テキストカラーを `slate-*` 系に統一

**`src/components/kanban/BacklogView.tsx`**
- リストアイテム背景・ボーダーを `slate-*` 系に統一

**`src/components/ui/Button.tsx`**
- Secondary バリアント: `bg-gray-200` → `bg-slate-200`、`hover:bg-gray-300` → `hover:bg-slate-300`

---

## Phase 3: カードデザイン統一

**`src/components/kanban/TaskCard.tsx`**
- `rounded-md` → `rounded-xl`（他カードと統一）

**`src/components/kanban/StorySwimlane.tsx`**
- `rounded-lg` → `rounded-xl`

**`src/components/kanban/StatusColumn.tsx`**
- シャドウを `shadow-sm` に統一

---

## Phase 4: テキストオーバーフロー対応

**`src/components/kanban/StorySwimlane.tsx`**
- `<h2>` タグに `truncate` クラスと `title={story.title}` を追加
- 長いストーリータイトルによる2行崩れを解消

**`src/components/kanban/BacklogView.tsx`**
- アイテムタイトルに `line-clamp-2` を適用（最大2行で省略）

**ステータスバッジ（各所）**
- `whitespace-nowrap` を追加し、バッジ内テキストの折り返しを防止

---

## Phase 5: ヘッダーリファイン

**`src/App.tsx`**

| 変更 | 内容 |
|------|------|
| `LlmUsagePill` をヘッダーから削除 | `Board` コンポーネントに props 経由で移動 |
| 履歴ボタンをヘッダーから削除 | `Board` コンポーネントに props 経由で移動 |
| ヘッダー右クラスター | `[プロジェクト名▼ | フォルダ⚙]` のみに整理 |

**`src/components/kanban/Board.tsx`**
- スプリントボードのヘッダー行右側に `LlmUsagePill` と 履歴ボタンを配置

**`src/components/kanban/ScrumDashboard.tsx`**
- `Board` に渡す props (`projectId`, `onOpenHistory`) の中継を追加

変更後のヘッダー構造:
```
[Vicara ロゴ]  [Inception | Kanban | Settings]  [プロジェクト名▼ | フォルダ⚙]
```

変更後の Board ヘッダー行:
```
スプリントボード          [コスト] [履歴] [動作確認 or 停止]
Sprint #X  2025/04-05
```

---

## Phase 6: 動作確認ボタン整理

**`src/components/kanban/Board.tsx`**

| 変更前 | 変更後 |
|--------|--------|
| `min-h-[56px]` の3行カード型ボタン | `h-10` の標準ボタン1個 |
| `sky-*` カラー | `blue-*` カラーに統一 |
| `rounded-2xl` | `rounded-xl` |
| ボタン内にURL表示 | `title` 属性（ホバー tooltip）に移動 |
| 起動中に別の「停止」ボタンが出現 | 同じボタンが2状態（起動 / 停止）に切替 |

停止ボタンが突然出現してレイアウトが動的に変化する問題が解消される。

---

## Phase 7: Dev Agent / PO アシスタント 動線改善

**`src/components/terminal/TerminalDock.tsx`**
- 最小化状態（34px帯）の全体をクリック可能なボタンに改修
- アイコン + "Dev Agent" ラベル + "▲ 開く" テキストを配置
- 「ここをクリックすれば開く」が直感的に分かる

**`src/App.tsx`**
- 下部フロートハンドルのラベル: `チームの稼働状況` → `Dev Agent`
- 右端ハンドルのラベル: `PO アシスタント / ふせん` → `PO`
- エージェント稼働中: 下部ハンドルに `badge="●"` を渡す
- PO 未読メッセージあり: 右端ハンドルに `badge="●"` を渡す

バッジは静的なドット（アニメーションなし）。既存の `bg-blue-600` を流用。

---

## Phase 8: フォーカスリング統一

**`src/components/ui/Button.tsx`**, **`Input.tsx`**, **`Textarea.tsx`**, **`Modal.tsx`**
- `focus:ring-offset-2` が抜けている箇所に追加
- Tab キーでの画面操作時の視認性が向上

---

## Phase 9: Badge コンポーネント化

**`src/components/ui/Badge.tsx`**（新規作成）
- 優先度バッジ（P1〜P5）とステータスバッジの共通実装を提供

**`src/components/kanban/TaskCard.tsx`**
- インライン定義の優先度バッジを `<Badge>` に置換

**`src/components/kanban/StorySwimlane.tsx`**
- インライン定義の優先度バッジを `<Badge>` に置換（重複コードの解消）

---

## 変更しないもの

- すべての機能ロジック（スプリント管理・タスク操作・AI チャット・ターミナル実行）
- `frontend-core` 配下の型定義・Context・Hooks（参照のみ）
- バックエンド（Rust / Tauri コマンド）
- 設定画面のカードデザイン（`ProviderCard` は適切なデザインのため維持）
