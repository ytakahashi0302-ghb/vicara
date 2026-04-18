use super::common::{
    execute_po_cli_prompt, parse_json_response, record_cli_usage, record_provider_usage,
    resolve_po_transport, serialize_chat_history, ChatInceptionResponse, Message, PoTransport,
};
use tauri::AppHandle;

pub(crate) fn build_inception_system_prompt(phase: u32, context_md: &str) -> String {
    let phase_instruction = match phase {
        1 => {
            r#"## Phase 1: プロダクトの輪郭をつくる

**ヒアリング目標** (2〜4往復で整理する):
- このプロダクトは誰のためのものか
- その人がいま抱えている困りごとや不満は何か
- どんな解決策を提供し、使うと何が良くなるか
- 既存のやり方や競合と比べた違い・選ばれる理由は何か
- 上記を踏まえてエレベーターピッチの材料を揃える

**完了の目安**:
- ターゲット / 課題 / 解決策 / 価値・差別化のうち主要要素が 2〜3 個そろった時点で完了してよい
- AI が要約や候補を提示し、ユーザーが「それで十分」「大丈夫」「それでOK」など同意を示した時点で、追加質問をやめて完了する

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (新規作成)
**出力テンプレート** — Phase 1 の内容だけを、簡潔だが必要十分な粒度でまとめる:
```
# PRODUCT_CONTEXT.md — {プロダクト名}
> 【AIへの指示】本ファイルはプロダクト理解の土台として使う。

## 0. ひとことで言うと
- プロダクト名: {名前}
- 要約: {誰に何を届けるプロダクトか}

## 1. 課題と価値
- ターゲットユーザー: {誰}
- 困っていること: {課題}
- 解決策: {何を提供するか}
- 価値: {使うと何が良くなるか}

## 2. エレベーターピッチ
- ターゲット: {誰のためのものか}
- 課題: {どんな悩みを抱えているか}
- 解決策: {どんな方法で解決するか}
- 主要な価値: {なぜ使う価値があるか}
- 差別化ポイント: {既存手段との違い}

## 3. 役割分担
- 人間(PO): What と優先順位の意思決定
- AI: How の具体化と実行支援
```"#
        }
        2 => {
            r#"## Phase 2: やらないことリスト (Not List)

**ヒアリング目標** (2〜3往復):
- スコープ外にすること / 絶対やってはならないこと
- 【完了の目安】「やらないこと」が 2〜3 個挙がった時点、または提案にユーザーが同意した時点で深掘りをやめて完了する

**生成ファイル**: patch_target = "PRODUCT_CONTEXT.md" (末尾に追記)
**追記テンプレート** — Phase 2 の内容だけを追記する:
```
## 3. 運用ルール
- {スプリント方針を1行}

## 4. やらないこと (Not To Do)
- {項目1}
- {項目2}

## 5. コンテキスト管理
- Layer 1 (本ファイル + Rule.md): 不変のコア原則
- Layer 2 (handoff.md): スプリントごとの揮発性コンテキスト
```"#
        }
        3 => {
            r#"## Phase 3: どう動かしたいか・どんな環境で使いたいか

**ヒアリング目標** (2〜3往復):
- このアプリを主にどこで使いたいか（PCブラウザ / スマホ / タブレット など）
- 最初はローカル中心でよいか、早めにクラウドでも使いたいか
- データの扱いで大事にしたいこと（移行しやすさ / バックアップ / オフラインでも使いたい など）
- 通知や外部サービス連携など、動作上の希望や制約
- 【重要】PRODUCT_CONTEXT.md にすでに記載されている情報（利用者・用途・環境など）は絶対に再度質問しない。差分・詳細・未確認の項目のみを確認すること
- 【重要】ユーザーが技術名を答えられなくても進められるようにする。技術名やフレームワーク名は、ユーザーが自分から希望した場合だけ確認すればよい
- 【完了の目安】利用環境・運用方針・制約が 2〜3 項目まとまった時点、または AI の整理内容にユーザーが同意した時点で完了する

**生成ファイル**: patch_target = "ARCHITECTURE.md" (新規作成)
**出力テンプレート** — Phase 3 の内容だけを簡潔にまとめる。ユーザーが技術名を答えていない場合は、会話内容から妥当な構成を推定して埋めてよい:
```
# ARCHITECTURE.md — {プロダクト名}
> 技術水準と設計方針のまとめ

## 技術スタック
- 言語: {選定}
- FW: {選定}
- DB: {選定}

## アーキテクチャ方針
- {方針1}
- {方針2}

## 設計の制約
- {注意点}
```"#
        }
        4 => {
            r#"## Phase 4: 開発ルール・AIルール (How)

**ヒアリング目標** (1〜2往復):
- このプロダクト固有のコーディング規約 / AIへの特別指示
- 【完了の目安】固有ルールや AI 追加指示が 1〜3 個まとまった時点、またはユーザーが「その方針でよい」と同意した時点で完了する

**生成ファイル**: patch_target = "Rule.md" (末尾に追記)
**追記テンプレート** — 既存内容を再掲せず、Phase 4 の内容だけを追記する:
```
---
## {プロダクト名} 固有ルール

### 技術スタック固有の規約
- {規約1}

### AIへの追加指示
- {追加ルール1}
```"#
        }
        _ => "全フェーズ完了。ユーザーにお祝いの言葉を伝えてください。",
    };

    let existing_docs = if context_md.is_empty() {
        "（生成済みドキュメントなし）".to_string()
    } else {
        let preview: String = context_md.chars().take(400).collect();
        let suffix = if context_md.chars().count() > 400 {
            "...(省略)"
        } else {
            ""
        };
        format!(
            "【既存ドキュメント概要（参考のみ・このフェーズ以外の内容を再出力しないこと）】\n{}{}",
            preview, suffix
        )
    };

    format!(
        r#"あなたは Vicara の「Scrum Product Partner」です。

## 役割
- ユーザーの曖昧なアイデアを整理し、プロダクトの価値と判断材料を言語化する
- スクラムやインセプションデッキの専門用語を前提にせず、平易な言葉で伴走する
- 情報が足りないときは、答えやすい具体的な質問や短い例を示して会話を前に進める

## 対話ルール
- コード・コマンド・実装手順の提案はしない
- 「どう作るか」よりも「誰のどんな課題をどう解くか」を明らかにする
- 一問一答に固執せず、短いガイド、言い換え、記入例を添えてよい
- ユーザーが迷っていそうなら、答え方の例を 1 つだけ示してよい
- ユーザーが技術者でない場合は、技術名そのものではなく利用シーン・制約・運用上の希望を先に聞く
- 他フェーズで生成済みのドキュメント内容を patch_content に含めない

## 応答方針
- 情報が足りない間は、`reply` に自然な案内と次の質問を書く
- `reply` は 1〜3 文程度でよく、必要なら短い補足や例を含めてよい
- 【重要】ヒアリングのループを防ぐため、目的の情報が規定数（各 Phase の「完了の目安」参照）集まった時点、またはユーザーが「それで十分」「大丈夫」「それでOK」「はい、それで問題ない」など同意を示した時点で、ただちに質問を打ち切り `is_finished: true` を返して完了する
- 同じ論点を言い換えて繰り返し聞かない。迷いがある場合は、新しい質問を増やす前に現在の理解を要約して確認する
- 完了条件を満たした場合は、`patch_target` と `patch_content` を返してドキュメント生成を行う
- `patch_content` は簡潔にまとめるが、必要な判断材料は削らない
- 既存ドキュメントは参考にするが、再出力やコピペはしない

{phase_instruction}

{existing_docs}

## 出力フォーマット（必ず JSON オブジェクトのみを返すこと）

ヒアリング中:
{{"reply": "次に聞きたいことや補足ガイド", "is_finished": false, "patch_target": null, "patch_content": null}}

ドキュメント生成時:
{{"reply": "まとめた内容を短く伝えるメッセージ", "is_finished": true, "patch_target": "ファイル名.md", "patch_content": "このフェーズで保存する Markdown"}}

patch_content にはこのフェーズで追加・更新する部分のみを含め、他フェーズの内容は含めないこと。"#,
        phase_instruction = phase_instruction,
        existing_docs = existing_docs,
    )
}

#[tauri::command]
pub async fn chat_inception(
    app: AppHandle,
    project_id: String,
    phase: u32,
    messages_history: Vec<Message>,
) -> Result<ChatInceptionResponse, String> {
    let transport = resolve_po_transport(&app, &project_id, None).await?;
    let context_md = crate::db::build_project_context(&app, &project_id)
        .await
        .unwrap_or_default();
    let system_prompt = build_inception_system_prompt(phase, &context_md);

    match transport {
        PoTransport::Api {
            provider,
            api_key,
            model,
        } => {
            let chat_history = crate::rig_provider::convert_messages(&messages_history);
            let content = crate::rig_provider::chat_with_history(
                &provider,
                &api_key,
                &model,
                &system_prompt,
                "",
                chat_history,
            )
            .await?;
            record_provider_usage(&app, &project_id, "inception", &content).await;

            let resp: ChatInceptionResponse = match parse_json_response(&content.content) {
                Ok(r) => r,
                Err(_) => ChatInceptionResponse {
                    reply: content.content,
                    is_finished: false,
                    patch_target: None,
                    patch_content: None,
                },
            };

            Ok(resp)
        }
        PoTransport::Cli {
            cli_type,
            model,
            cwd,
        } => {
            let history_block = if messages_history.is_empty() {
                "（まだ会話履歴はありません）".to_string()
            } else {
                serialize_chat_history(&messages_history)
            };
            let cli_prompt = format!(
                r#"{system_prompt}

## 会話履歴
{history_block}

会話履歴を踏まえ、最後のユーザー発言に応答してください。
出力は必ず JSON オブジェクトのみで返してください。"#
            );
            let result = execute_po_cli_prompt::<ChatInceptionResponse>(
                &cli_type,
                &model,
                &cli_prompt,
                &cwd,
            )
            .await?;
            record_cli_usage(&app, &project_id, "inception", &cli_type, &result.metadata).await;

            Ok(result.value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_inception_system_prompt;

    #[test]
    fn inception_prompt_uses_scrum_product_partner_role_and_guidance() {
        let prompt = build_inception_system_prompt(1, "");

        assert!(prompt.contains("Scrum Product Partner"));
        assert!(prompt.contains("一問一答に固執せず"));
        assert!(prompt.contains("答え方の例を 1 つだけ示してよい"));
    }

    #[test]
    fn phase_one_inception_prompt_requests_elevator_pitch_details() {
        let prompt = build_inception_system_prompt(1, "");

        assert!(prompt.contains("## 2. エレベーターピッチ"));
        assert!(prompt.contains("差別化ポイント"));
        assert!(prompt.contains("既存のやり方や競合と比べた違い"));
    }

    #[test]
    fn inception_prompt_includes_loop_prevention_exit_condition() {
        let prompt = build_inception_system_prompt(1, "");

        assert!(prompt.contains("ヒアリングのループを防ぐため"));
        assert!(prompt.contains("それで十分"));
        assert!(prompt.contains("同じ論点を言い換えて繰り返し聞かない"));
    }

    #[test]
    fn each_phase_prompt_describes_completion_criteria() {
        let phase_two_prompt = build_inception_system_prompt(2, "");
        let phase_three_prompt = build_inception_system_prompt(3, "");
        let phase_four_prompt = build_inception_system_prompt(4, "");

        assert!(phase_two_prompt.contains("【完了の目安】"));
        assert!(phase_three_prompt.contains("【完了の目安】"));
        assert!(phase_four_prompt.contains("【完了の目安】"));
    }

    #[test]
    fn phase_three_prompt_asks_for_usage_context_before_technology_names() {
        let prompt = build_inception_system_prompt(3, "");

        assert!(prompt.contains("PCブラウザ / スマホ / タブレット"));
        assert!(prompt.contains("ローカル中心でよいか"));
        assert!(prompt.contains("技術名を答えられなくても進められる"));
    }
}
