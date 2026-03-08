# マイクロスクラム・エンジン（8時間スプリントタイマー機能）実装計画

マイクロスクラム（1スプリント＝最大8時間）の進行を管理するためのタイマー機能と、コード品質を担保するための仕組み（ESLint）を実装します。

## 実装方針

### 1. アプリ再起動でもズレないタイマーの設計 (State Management)
タイマーの「現在状態」「残り時間」「直近の開始時刻」をローカルに永続化し、アプリ再起動時にも正確に復元できるようにします。

- **永続化手法**: `tauri-plugin-store` を利用して `sprint.json` に状態を保存します（単純なKey-ValueなのでDBテーブル追加より一時的な状態管理としてStoreが適しています）。
- **ステータス定義**:
  - `NOT_STARTED`: 初期状態（残り8時間 = 28,800,000 ms）
  - `RUNNING`: 実行中（経過時間を `Date.now() - started_at` で実時間計算）
  - `PAUSED`: 一時停止中（残り時間は固定保存）
  - `COMPLETED`: 完了
- **計算ロジック**:
  - 実行中の場合、常に `(保存された残り時間) - (Date.now() - 保存された開始時刻)` を現在時刻として再計算します。これにより、アプリを終了していてもバックグラウンドで時間が経過しているように振る舞うことができ、実時間を厳密にトラッキングします。

### 2. タイマーUIの構築
- **SprintTimer コンポーネント作成**: `src/components/SprintTimer.tsx`
  - 画面上部（`App.tsx` のヘッダーや上部領域）に配置。
  - プログレスバー：全8時間のうち、残り時間の割合を視覚的に表示（色を緑→黄→赤と変化させる）。
  - カウントダウン： `07:59:59` 形式で表示。
  - コントロール：「開始」「一時停止」「完了」「リセット」ボタンを配置。

### 3. デイリースクラム（1時間ごとの通知）機能
- `SprintTimer` 内部の時間更新処理 (setInterval) で、残り時間が「1時間（3,600,000 ms）」ごとの境界をまたぐタイミングで通知を発火します。
- **通知UI**: 画面右下にトースト通知風のポップアップ（HTML/CSS）を実装し、「1時間が経過しました。現在のタスクの進捗は順調ですか？」と表示します。
- 重複通知を防ぐため、既に通知済みの時間（残り7時間、6時間...）を状態として保持し、保存も行います（不意の再起動でも同じ通知が出ないようにするため）。

### 4. ESLintの本格導入 (Tech Debt)
- 実装を開始する前に、`eslint`, `@typescript-eslint/eslint-plugin`, `eslint-plugin-react-hooks` のセットアップを行います。
- `eslint.config.js` などの設定ファイルを作成し、`npm run lint` コマンドを TypeScript + ESLint のチェックが行われるようにアップデート。既存コードのエラーがあれば修正します。

## User Review Required

> [!IMPORTANT]
> - タイマーの永続化には SQLite ではなく `tauri-plugin-store` (ローカルストア: sprint.json) を用いる方針で問題ないでしょうか？（過去の履歴をDB等に残す要件ではなく、現在のタイマー状態を維持することが目的であるため Store が最適と考えています）
> - この実装計画でよろしければ、GOサインをお願いいたします。

## Proposed Changes

### Configuration/Setup
#### [NEW] eslint.config.js
ESLint の設定ファイルを追加。
#### [MODIFY] package.json
`eslint` パッケージの追加および `lint` スクリプトの修正。

### Frontend
#### [NEW] src/components/SprintTimer.tsx
スプリントタイマーのUIおよび完了・1時間ごとの通知表示。
#### [NEW] src/hooks/useSprintTimer.ts
タイマーの再計算・開始・停止・Storeへの保存ロジックを切り出したカスタムフック。
#### [MODIFY] src/App.tsx
カンバン上部などに `SprintTimer` コンポーネントを組み込み。

## Verification Plan

### Automated Tests
- `npm run lint` がエラーゼロで通過することを確認。
- `npm run build` が成功することを確認。

### Manual Verification
- 「Sprint Start」でカウントダウンが始まること。
- アプリを再起動（またはリロード）しても、開始時刻から正しく差分計算されてカウントダウンが継続・復元すること。
- 「Pause」「Complete」で正しくタイマーが停止・完了状態になること。
- スプリント実行中、タイマーの表示が1時間分減った際に通知が表示されること。

## 追加要件: 動的スプリント時間とスマート通知 (Sprint Duration Update)

### 1. スプリント時間の可変化設計
- **SettingsModal**:
  - `sprint-duration-hours` として設定値（1h, 2h, 4h, 8h等）を `settings.json` に保存します。
  - デフォルト値はAI開発に合わせた「1時間」とします。
- **Store構造の統合（独立性の担保）**:
  - 進行中（`RUNNING` または `PAUSED`）のスプリント中に、Settings側でスプリント時間が変更されても不整合が起きないよう、スプリント開始時点 (`startSprint` 呼び出し時) で設定を読み込み、`sprint.json` の `SprintState` に `durationMs` としてスプリント開始時の総時間を保存します。

### 2. タイマーUIのアフォーダンス動的化
これまでの「残り4時間・2時間」という固定値から、全体に対する割合（％）へ変更します。
- **Warning**: 残り時間が全体の `50%` 未満になったら黄色
- **Late**: 残り時間が全体の `10%` 未満になったら赤色

### 3. スマートな通知タイミング（折り返し地点）
- `SprintState` から `notifiedHours: number[]` を削除し、代わりに `hasNotifiedHalfway: boolean` を追加します。
- 経過時間が `durationMs / 2` を超えたタイミングで、「折り返し地点です。進捗は順調ですか？」というトーストUIを発火し、`hasNotifiedHalfway = true` に保存して重複を防止します。

## 追加要件の User Review Required

> [!IMPORTANT]
> 進行中のスプリントと設定値の切り離し設計：
> スプリント稼働中に設定画面で「Sprint Duration」を変更した場合、**現在走っているスプリントの時間は変わらず、次回の「Start Sprint」時から新しい時間が適用される**という堅牢な設計を提案していますが、この挙動でよろしいでしょうか？
> この計画変更にGOサインをいただけましたら、修正作業に入ります。

## UI表示バグの修正 (NOT_STARTED状態のリアクティビティ)

**【原因の特定】**
現在の `useSprintTimer` は、初期化時や `NOT_STARTED` 状態の維持時に、直前のスプリント状態が保存されている `sprint.json` の `durationMs` をそのまま画面に表示しています。`SettingsModal` で `settings.json` が更新されても、`sprint.json` 側には通知がいかず、**次に「Start Sprint」を押したタイミングで初めて設定を読みに行く**仕様になっているため、UIの初期表示が古いまま取り残されてしまっていました。また、`resetSprint` の際も、Store上の古い `durationMs` を使用してリセットしているため、設定変更後にResetを押しても新しい時間が反映されません。

**【修正方針】**
以下の3箇所を改修し、状態の完全な同期とリアクティビティを担保します。

1. **設定変更の通知 (`SettingsModal.tsx`)**
   - 設定の保存(`handleSave`)完了時に、アプリ全体に対して `window.dispatchEvent(new CustomEvent('settings-updated'))` を発行し、設定が変わったことを通知します。

2. **最新設定の同期関数の追加 (`useSprintTimer.ts`)**
   - `settings.json` から最新の `sprint-duration-hours` を読み込んでミリ秒（durationMs）を返す非同期処理を共通化します。

3. **反映ロジックの適応 (`useSprintTimer.ts`)**
   - **イベントリスナー**: `settings-updated` イベントを受信した際、現在の状態が `NOT_STARTED` であれば、最新の時間を読み直して `durationMs` と `remainingTimeMs` を更新し、画面に即座に反映します。
   - **初期ロード**: アプリ起動時（`initStore`）、読み込んだ状態が `NOT_STARTED` の場合は、以前の `sprint.json` の時間ではなく最新の `settings.json` の時間を使って初期化します。
   - **リセット処理**: `resetSprint()` が呼ばれた際にも最新の時間を読み込んでリセット状態を作ります。

この実装により、「実行中スプリントの時間は変わらない（保護される）」という要件を守りつつ、「待機中のタイマー表示は即座に最新設定を反映する」という正しいUXを実現できます。
