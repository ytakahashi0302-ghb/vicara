# フロントエンドDB操作レイヤーおよびカンバンUIの構築 (Walkthrough)

## 実装内容
1. **Rule.md の更新**:
    - `05. 命名規則・採番ルール` を追記し、以後の機能・ドキュメントディレクトリ群にプレフィックス（`01_`, `02_` 等）を付けるルールを適用しました。
    - 既存の `docs/ai_scrum_tool_init` を `docs/01_ai_scrum_tool_init` にリネームしました。

2. **Tailwind CSS および UIパッケージの導入**:
    - `tailwindcss`, `postcss`, `autoprefixer` を導入し、設定ファイル（`tailwind.config.js`, `postcss.config.js`, `App.css`）を構成しました。
    - ドラッグ＆ドロップ実装のために `@dnd-kit/core`, `@dnd-kit/sortable`, `@dnd-kit/utilities` を導入しました。
    - アイコン表示用に `lucide-react` を導入しました。

3. **型定義とDB操作レイヤー (Hooks) の実装**:
    - `src/types/index.ts` に、SQLiteスキーマに準拠した `Story` および `Task` のインターフェースを作成しました。
    - `@tauri-apps/plugin-sql` を使用して、プレースホルダー（`$1`, `$2`）を用いたCRUD処理を行うカスタムフックを作成しました。
        - `useDatabase.ts` (DB接続)
        - `useStories.ts` (StoryのCRUD)
        - `useTasks.ts` (TaskのCRUD)

4. **状態管理 (Context) の実装**:
    - `src/context/ScrumContext.tsx` を作成し、DBから取得したデータをアプリケーション全体で状態同期・管理できるようプロバイダーでラップしました。

5. **共有コンポーネントとカンバンUIの実装**:
    - 汎用コンポーネントとして `Button.tsx`, `Card.tsx` を作成しました。
    - POの要望（A案）に沿ったカンバンボードUIを構築しました。
        - **スウィムレーン形式**: 縦軸にStory、横軸にTaskステータス（ToDo, In Progress, Done）を配置する `StorySwimlane.tsx` と `StatusColumn.tsx` を作成しました。
        - **DnD機能実装**: `TaskCard` をドラッグ可能（Sortable）にし、`Board.tsx` にて `onDragEnd` イベントをハンドリングしました。
        - **制約（Plan A）**: ドラッグ元と異なるStoryレーンへのドロップは許可しない（移動先が同じ `story_id` を持つ行のみ更新を実行する）ロジックを組み込みました。

6. **メイン画面統合**:
    - `src/App.tsx` を改修し、ダミーデータ（モック）を投入してDBの書き込みと画面更新・ドラッグ確認を行うための `DeveloperTools` モジュールを一時的に配置しました。

## テスト結果と確認事項（Manual Verification Request）

フロントエンド実装およびDB層との連携コードが完了し、`npm run tauri dev` コマンドにて開発サーバーを起動しました。
PO（ユーザー様）にて以下の動作確認をお願いいたします。

### 動作確認手順
1. アプリケーションが起動し、グレー背景の新しいUI（MicroScrum AI）が表示されることを確認してください。
2. 最初は「No Stories Yet」と表示されます。
3. 画面右下にある **[Add Mock Data]**（Dev Tools）ボタンをクリックしてください。
    - ※ これにより `hooks/` を経由してローカルの `ai-scrum.db` へテスト用のStoryとTaskが書き込まれます。
4. 画面が更新され、Jiraのような「Storyごとの横長のスウィムレーン」が表示されることを確認してください。
5. **ドラッグ＆ドロップ制約テスト**:
    - （A）Taskカードをドラッグし、**同じ横レーン内の**別のステータス（例: To Do -> In Progress）へドロップした際、移動が完了しステータスが変わることを確認してください。
    - （B）この変更がアプリを再起動しても維持されていること（DBへのUPDATE完了）を確認してください。
    - （C）（現時点の仕様上、UIの見た目で他レーンに置けるように見える場合がありますが）**別のStoryの横レーンへTaskをドロップしても、移動が無視され元の位置に戻ること**（A案の制約が効いていること）を確認してください。

上記が正しく動作すれば「フロントエンドDB操作レイヤーおよびカンバンUIの構築」フェーズは完了です！
問題なく動いたか、もしくはデザイン面などでのフィードバックがあればお知らせください。
