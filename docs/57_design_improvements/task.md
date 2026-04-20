# EPIC57 タスクリスト — デザインブラッシュアップ

## 概要
機能を損なわずUIのデザイン統一・動線改善を行う。

---

## Phase 1: デザイントークン定義
- [ ] `tailwind.config.js` にカスタムカラー・角丸・シャドウを定義
- [ ] `gray-*` → `slate-*` の統一方針を確定

## Phase 2: カラー統一
- [ ] `src/App.tsx` — `bg-gray-100` → `bg-slate-100` 等の置換
- [ ] `src/components/kanban/TaskCard.tsx` — gray/slate 混在を解消
- [ ] `src/components/kanban/StorySwimlane.tsx` — ヘッダー背景色統一
- [ ] `src/components/kanban/StatusColumn.tsx` — カラー統一
- [ ] `src/components/kanban/BacklogView.tsx` — リストアイテム背景統一
- [ ] `src/components/ui/Button.tsx` — Secondary の gray → slate 統一

## Phase 3: カードデザイン統一
- [ ] `TaskCard.tsx` — `rounded-md` → `rounded-xl` に統一
- [ ] `StorySwimlane.tsx` — `rounded-lg` → `rounded-xl` に統一
- [ ] `StatusColumn.tsx` — shadow・border 統一
- [ ] `BacklogView.tsx` — カードスタイル統一

## Phase 4: テキストオーバーフロー対応
- [ ] `StorySwimlane.tsx` — ストーリータイトルに `truncate` + `title` 属性追加
- [ ] `BacklogView.tsx` — アイテムタイトルに `line-clamp-2` 適用
- [ ] ステータスバッジに `whitespace-nowrap` 追加

## Phase 5: ヘッダーリファイン
- [ ] `LlmUsagePill`（コスト表示）をヘッダーから `ScrumDashboard` または `Board` ヘッダー行へ移動
- [ ] `履歴ボタン` を Kanban 画面内（Board ヘッダー行）へ移動
- [ ] ヘッダー右クラスターをプロジェクト名 + フォルダ設定のみに整理
- [ ] プロジェクトフォルダ設定（ProjectSettings）を Settings 画面に一本化するか検討

## Phase 6: 動作確認ボタン（Board プレビュー）の整理
- [ ] `Board.tsx` のプレビューボタンを標準ボタン1個に整理
  - 3行テキスト → 1行テキスト
  - `sky-*` → `blue-*` / `slate-*` に統一
  - `rounded-2xl` → `rounded-xl` に統一
  - URL表示をホバー tooltip へ移動
- [ ] 起動中は同じボタンが「停止」に切り替わる2状態設計に変更
- [ ] 別途出現する「停止ボタン」を廃止

## Phase 7: Dev Agent / PO アシスタント 動線改善
- [ ] ターミナル最小化バー（34px）をクリック可能な帯に改修
  - アイコン + "Dev Agent" ラベル + 展開ボタンを配置
  - バー全体がクリックでトグル
- [ ] `EdgeTabHandle`（下部フロート）のラベルを `チームの稼働状況` → `Dev Agent` に変更
- [ ] `EdgeTabHandle`（右端）のラベルを `PO アシスタント / ふせん` → `PO` に短縮
- [ ] Dev エージェント稼働中に下部ハンドルへバッジドット（●）を表示
- [ ] PO アシスタントに未読メッセージがある場合に右ハンドルへバッジドット表示

## Phase 8: フォーカスリング・インタラクション状態の統一
- [ ] `Button.tsx`, `Input.tsx`, `Textarea.tsx`, `Modal.tsx` で `focus:ring-offset-2` を統一追加

## Phase 9: バッジ・ステータス表示の統一
- [ ] `src/components/ui/Badge.tsx` を新規作成（優先度・ステータス共通）
- [ ] `TaskCard.tsx`, `StorySwimlane.tsx` の優先度バッジを `Badge` コンポーネントに置換
