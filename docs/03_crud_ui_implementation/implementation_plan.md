# データ入力モーダル・フォームの実装計画 (Phase 3)

## 概要
ハードコードされたモックデータではなく、ユーザーがUI上から直接StoryおよびTaskを追加・編集・削除できるようにする。入力にはモーダルウィンドウを使用し、モダンなフォームUX（Tailwind CSS + lucide-react）を提供する。

## User Review Required
> [!IMPORTANT]
> 以下の計画を確認し、承認（GOサイン）をお願いします。承認後に実際のコーディングを開始します。

## Proposed Changes

### [コンポーネント: 共通UI (UI Elements)]
#### [NEW] `src/components/ui/Modal.tsx`
- 汎用的なモーダルコンポーネント。背景オーバーレイ（Backdrop）と中央センタリング、閉じる（×）ボタン等の枠組みを提供する。

#### [NEW] `src/components/ui/Input.tsx`, `src/components/ui/Textarea.tsx`, `src/components/ui/Button.tsx` (必要に応じ)
- フォーム部品のエラー状態（未入力時の赤枠表示など）をサポートした汎用部品を用意し、Tailwind CSSでスタイリングする。

### [コンポーネント: Story関連機能]
#### [NEW] `src/components/board/StoryFormModal.tsx`
- **新規作成 / 編集 兼用**モーダル。
- **フィールド**:
  - `title` (必須バリデーション)
  - `description`
  - `acceptance_criteria`
- **機能**:
  - タイトルが空の場合は保存ボタンを押せない、またはエラーメッセージを表示するバリデーション。
  - 削除ボタン（編集モード時のみ下部にDangerなボタンとして表示）。

#### [MODIFY] `src/components/board/Board.tsx` (または適切な親コンポーネント)
- 画面上部に「Add Story」ボタン（lucide-reactのアイコンを活用）を配置し、クリックで `StoryFormModal` を開くステート管理（useState）を追加。

### [コンポーネント: Task関連機能]
#### [NEW] `src/components/board/TaskFormModal.tsx`
- **新規作成 / 編集 兼用**モーダル。
- **フィールド**:
  - `title` (必須バリデーション)
  - `description`
  - `status` (TODO, IN_PROGRESS, DONE など)
- **機能**:
  - 親StoryIDの紐づけ保持。
  - タイトル空チェック等のバリデーション。
  - 削除ボタン（編集モード時のみ）。

#### [MODIFY] `src/components/board/StorySwimlane.tsx` (またはヘッダー部)
- スウィムレーンの領域内に「Add Task」アイコンボタンを配置。ここから呼び出されるTaskフォームは、対象StoryのIDを初期値として保持する。

#### [MODIFY] `src/components/board/TaskCard.tsx` / `StorySwimlane.tsx` (カード表示部)
- 各カード内に編集用アイコン（例: Pencil または MoreVertical）を配置し、クリックで対象アイテムの編集モードとしてフォームモーダルを開く。

## テスト方針
### Automated Tests
- 本プロジェクトは現状エンドツーエンドテスト等の自動セットアップが未確認のため、基本的には型チェック(`tsc`)とフロントエンドのLinterエラー解消(`npm run lint`)を実行し、ビルドエラーが無いことを担保する。

### Manual Verification
1. 開発サーバー（`npm run tauri dev`）を起動。
2. **バリデーション確認**: タイトルを空にしたまま各フォームの保存を試み、エラーが出る（保存されない）ことを確認。
3. **Story追加**: 画面上部のボタンから正常なデータを入力して保存し、ボードに新しいスウィムレーンが追加されるか確認。
4. **Task追加**: 追加されたStory内のスウィムレーン上のボタンから子Taskを追加し、正しく指定Story内にカードが現れるか確認。
5. **編集**: 作成したStory/Taskの編集ボタンを押し、内容を変更して保存後、UIに即座に反映されるか確認。
6. **削除**: 編集モーダル内から削除ボタンを押し、ボード上から該当アイテムが消えるか（DBから消えるか）確認。
