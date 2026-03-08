# フロントエンドDB操作レイヤーおよびカンバンUIの構築

## 目的 (Goal Description)
`handoff.md` の次期ステップに基づき、SQLiteデータベースと連携するReactフロントエンドのDB操作レイヤー（カスタムフック）を構築し、共有コンポーネントを準備した上で、MVPのコアである「カンバン（ボード）UI」を実装します。これにより、Story（親）とTask（子）の強固な紐付けをGUI上で視覚的に管理・操作できるようになります。

## POレビュー事項 (User Review Required)
> [!IMPORTANT]
> 以下の点についてPO（ユーザー様）の承認をお願いします。
> - **状態管理**: `Rule.md` に従い、まずはReact標準のContext APIとStateを使用して状態管理を行いますがよろしいでしょうか。
> - **UIライブラリ**: カンバンのドラッグ＆ドロップ機能には `@dnd-kit/core` などを、UIアイコンには `lucide-react` を使用します。
> - **CSS（スタイリング）**: 効率的なUI構築のため `Tailwind CSS` を導入してスタイリングを行いますがよろしいでしょうか。

## 提案する変更 (Proposed Changes)

### フロントエンドパッケージ (Dependencies)
フロントエンドディレクトリにて、以下のパッケージを追加導入します。（Tauri側のDBプラグインは導入済み）
- `tailwindcss`, `postcss`, `autoprefixer` の導入と設定。
- `@dnd-kit/core`, `@dnd-kit/sortable`, `@dnd-kit/utilities` の導入（カンバン機能）。
- `lucide-react` の導入（アイコン汎用）。

### Types (型定義)
#### [NEW] `src/types/index.ts`
- DBマイグレーションスキーマに基づいた `Story` および `Task` のインターフェース定義（厳格な型定義）。

### DB操作レイヤー (Hooks & Context)
#### [NEW] `src/hooks/useDatabase.ts`
- `@tauri-apps/plugin-sql` を使用したDBインスタンス取得と基本接続用のフック。
#### [NEW] `src/hooks/useStories.ts`
#### [NEW] `src/hooks/useTasks.ts`
- CRUD操作（作成、読み込み、更新、削除）をカプセル化したカスタムフック。SQLインジェクション対策としてプレースホルダー（`$1`, `$2`等）の利用を徹底。
#### [NEW] `src/context/ScrumContext.tsx`
- アプリケーション全体でStoryとTaskの状態を共有・同期するためのProvider。

### 共有UIコンポーネント (Shared Components)
#### [NEW] `src/components/ui/Button.tsx`
#### [NEW] `src/components/ui/Card.tsx`
- ビジネスロジックを分離し、Tailwind CSSでスタイルされた純粋なUIコンポーネント群。

### カンバンビュー (Kanban Board)
#### [NEW] `src/components/kanban/Board.tsx`
- `dnd-kit` を機能拡張したドラッグ＆ドロップ領域（カンバン全体）の構築。
#### [NEW] `src/components/kanban/StoryColumn.tsx`
- 各Storyを親とする縦カラム（スウィムレーン）。タスクのステータス（Todo, In Progress, Done）を表現。
#### [NEW] `src/components/kanban/TaskCard.tsx`
- 個々のTaskを表すドラッグ可能なカードUI。

### メインビュー (App)
#### [MODIFY] `src/App.tsx`
- Viteの初期画面コードを削除し、`ScrumContext` プロバイダーとメインの `Board` コンポーネントを配置するよう改修。

## テスト方針 (Verification Plan)

### 自動テスト / 動作確認
- `npm run tauri dev` コマンドでアプリが正常にビルド・起動し、TypeScriptやLintのエラーがないことを確認。
- アプリケーションコンソールおよびターミナル上でエラーログが出力されないか検証。

### 手動検証 (Manual Verification)
以下のシナリオをユーザー環境にて確認いただきます。
1. **DB疎通確認**: アプリ起動時にエラーが発生せず、初期データ（または空データ）が正しく読み込まれるか。
2. **アイテム作成機能**: 画面上からStoryの新規作成、およびStoryに紐づくTaskの新規作成が正常に行え、画面に即座に反映されるか。
3. **ドラッグ＆ドロップ**: Taskカードをドラッグしてステータス（列）を移動した際、UI上で正しく反映されるとともに、再起動後もそのステータスが維持される（SQLiteに保存されている）か。
