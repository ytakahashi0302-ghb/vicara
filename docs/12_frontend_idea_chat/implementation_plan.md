# 実装計画: フロントエンド機能とAIアシスタントの結合 (12_frontend_idea_chat)

## 【変更前】初期の実装計画 (モーダルでの実装案)
*※以下の計画は初期の提案であり、現在は下の「改訂版」に沿って実装を進めています。*

ユーザーがAIと壁打ちし、要件を深掘りするためのチャットUIを構築し、前回実装したRust側の `refine_idea` コマンドと結合します。対話で固まった要件から直接Storyを作成（Pre-fill）できるシームレスなUXを提供します。

### UIのレイアウト構成
- **アクセス導線**: カンバンボード (`Board.tsx`) トップエリアの「ストーリーを追加」ボタンの横に「💡 アイデアから作成」ボタンを追加します。
- **チャットUI (IdeaRefinementModal.tsx)**:
  - 既存の `Modal` コンポーネントを活用した専用モーダル。
  - **メッセージ表示エリア**: ユーザーとAIのメッセージ吹き出しをスクロール可能なリスト領域で表示。
  - **入力エリア**: テキストエリアと「送信」ボタン。AI処理中（ローディング中）は入力を無効化し、スピナーを表示するなど直感的なUIとします。
  - **アクションエリア**: チャットが進行し、要件がまとまったとユーザーが判断したあとに押せる「この内容でStoryを作成する」アクションボタン。

### 既存のStory作成フローへの繋ぎ込み方
1. `Board.tsx` に状態 `isIdeaRefinementModalOpen` と `storyFormInitialData` （Storyの初期値）を新規追加します。
2. 「この内容でStoryを作成する」アクションが呼ばれた際、チャットで得られた対話記録を結合したもの、またはAIの最新の回答を `description` 等にセットした `Partial<StoryFormData>` を生成します（まずはチャットログのコピーを含める簡易な形とし、UXを確保します）。
3. `IdeaRefinementModal` 側から `onComplete(data)` を発火させモーダルを閉じ、同時に `storyFormInitialData` にセットした上で既存の `StoryFormModal` を開きます。
4. `StoryFormModal` の `initialData` Props機能により、チャット内容が事前入力された状態でフォームが表示されます。

---

## 【改訂版】実装計画: サイドドロワーとライブドキュメントによるモダンAIアシスタントUX

### 概要
ユーザーからのフィードバックに基づき、IdeaRefinementModal を廃止し、よりモダンでシームレスな「サイドドロワー型」のAIアシスタントUXへ刷新します。チャットをしながら、隣のペインで「要件定義ドキュメント（StoryのDraft）」がリアルタイムに組み上げられていくCopilotのような体験を実現します。合わせて、途中途切れバグを解消するためにLLM側のトークン数制限を引き上げます。

### アーキテクチャと設計方針

#### 1. バックエンド (Rust/Tauri) の改修
- **対象ファイル**: `src-tauri/src/ai.rs`
- **変更内容**:
  - `max_tokens` (Anthropic) および `maxOutputTokens` (Gemini) を `300` から `2000` に大幅引き上げ。
  - プロンプトの改修: LLMに対して「JSON形式のみ」で回答するよう指示。
    - 出力形式の例: `{"reply": "対話用メッセージ", "draft": "Storyのマークダウン形式の草案"}`
  - rust側で出力されたJSON文字列をパースし、必要に応じてプレーンなフォーマットに整えた上で、構造化データ（`RefinedIdeaResponse` 構造体など）としてフロントエンドに返却します。
  - **[重要追加]**: LLMがMarkdownブロック (```json ... ```) で囲って返してくるケースを想定し、JSONパースを行う前に、正規表現を用いたMarkdown装飾のクリーニング処理（抽出処理）を必ず挟みます。

#### 2. サイドドロワーUIの基本構造
- **対象ファイル**: `src/components/ai/IdeaRefinementDrawer.tsx` (新規)
- **変更内容**:
  - 画面の右端からスライドインする Drawer UI を実装します。幅は画面の半分〜2/3程度(`w-2/3` や `max-w-4xl`など)を確保します。
  - ドロワー内を横並び（またはフレックス）の2ペイン構成とします。
    - **左ペイン (Chat)**: ユーザーとAIのチャット履歴表示。これまでのModal内のUIを踏襲。
    - **右ペイン (Live Document)**: AIから返却された `draft` (Markdown) の最新状態をプレビュー表示するエリア。
  - チャット入力時に `invoke('refine_idea')` を呼び出し、レスポンスの `reply` をチャット履歴に、`draft` をライブドキュメントに即時反映させます。

#### 3. Story作成フローへの接続 (Handoff)
- ライブドキュメントエリア（またはドロワー下部）に「Storyとしてカンバンに追加」ボタンを配置。
- ボタン押下時、現在の `draft` の内容を `StoryFormModal` の `description` にセットした状態で渡し、ドロワーを閉じます。

#### 4. 既存コードからの置き換え
- **対象ファイル**: `src/components/kanban/Board.tsx`
- **変更内容**:
  - `IdeaRefinementModal` のインポートおよびコンポーネント呼び出しを削除。
  - 新規の `IdeaRefinementDrawer` に置き換え、開閉状態のStateとプロパティの繋ぎこみを行います。

### Proposed Changes

#### `src-tauri/src/ai.rs`
[MODIFY] `src-tauri/src/ai.rs`
- `refine_idea` コマンドで戻り値を構造体（JSONシリアライズ）に変更。
- GeminiとAnthropicそれぞれのシステムプロンプト＆呼び出しパラメータ（Max Token）の修正。
- 返答のJSONテキストからマークダウン装飾を取り除くクリーニング処理の追加。

#### `src/components/ai/IdeaRefinementDrawer.tsx`
[NEW] `src/components/ai/IdeaRefinementDrawer.tsx`
- サイドドロワー型・2ペイン構成のUIコンポーネント。

#### `src/components/ai/IdeaRefinementModal.tsx`
[DELETE] `src/components/ai/IdeaRefinementModal.tsx`
- 旧モーダルUIは削除。

#### `src/components/kanban/Board.tsx`
[MODIFY] `src/components/kanban/Board.tsx`
- Drawerの呼び出しへの差し替え。

### Verification Plan（検証計画）
- `cargo check` と `npm run lint` が通ることを確認。
- `npm run dev` にて起動後、「アイデアから作成」ボタンを押下してドロワーが画面右からスライドインすることを確認。
- チャットを送信すると、AIの思考（ローディング）後に「左ペインに返信」「右ペインにマークダウンのドキュメント」が正しく分かれて表示されることを確認。
- 長文の出力要求を行なっても、回答が途切れないこと（Max Token 2000の効力）を確認。
