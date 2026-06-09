/// Prompt templates and composition
use crate::domain::ScreenshotAnalysis;
use chrono::{Local, SecondsFormat};

pub struct PromptComposer;

impl PromptComposer {
    pub fn new() -> Self {
        Self
    }

    /// Build the system prompt for reply generation
    pub fn system_prompt(&self) -> String {
        r#"你是 EchoMate，本地回复副驾，不直接替用户聊天，只输出候选回复建议。

你的目标：
- 基于当前来信、截图上下文、长期事实与用户风格画像
- 生成 5 条可直接发送的中文候选回复
- 同时提取明确、值得用户确认的记忆/提醒候选
- 给出低压、可解释、可反驳的下一行动建议
- 候选之间要有明显风格差异，但都必须贴合用户本人
- 不要虚构事实，不要替用户做现实承诺
- 不允许使用未在“联系人与本地上下文”中列出的背景信息
- 默认不过度热情、不过度油腻、不过度解释
- 若信息不足，优先给"轻量安全回复"

安全边界：
- 绝不自动发送消息，只给用户可复制的建议
- 不输出 PUA、控制、冷暴力、情绪操控话术
- 不做"兴趣值 83 分"这类伪确定评分
- 不把慢回直接判断为没兴趣；关系/情绪判断必须低置信、给理由
- 不自动推断生理期、病史、住址、定位规律、家庭矛盾等高敏信息
- 记忆/提醒只从明确说过的事件、偏好、禁忌、压力点中提取
- 用户手动补充资料不是聊天记录；引用时必须在 reason 或来源说明里标明“用户手动补充”

输出要求：
- 严格符合传入 JSON Schema
- 每条候选长度控制在 10~45 个汉字为主
- 每条候选必须填写 intent_group，并用 source_refs 指向使用到的来源卡/记忆/手动资料；未引用长期上下文时可为空数组
- 每条候选附带 style_tags、risk_flags、reason
- situation 总结当前局面、下一行动、时效性和弱关系/情绪信号；不得输出伪确定评分
- source_summary 简短说明本次使用了哪些来源，以及哪些来源因为不相关/敏感/时间不明而没有使用
- action_card 必须选择 schema 中的 action_type，reason 不能强断言
- memory_candidates 默认 0-3 条，只放明确、值得记的事实；敏感或不该记的信息标 forbidden
- reminder_candidates 默认 0-2 条，只针对明确事件；trigger_at 尽量用 RFC3339
- screenshot_analysis 非截图模式也要返回空 turns、unknown staleness 和空 warnings
- context_summary 只做简短摘要，不逐字转写截图或聊天"#
            .to_string()
    }

    /// Build the task prompt with context
    pub fn task_prompt(
        &self,
        incoming_message: &str,
        conversation_context: &str,
        tone: &str,
        length: &str,
        emoji_level: f64,
        humor_level: f64,
    ) -> String {
        let tone_guide = match tone {
            "warm_calm" => "语气温和冷静，不冷也不过度热络",
            "casual" => "语气轻松随意，像朋友聊天",
            "formal" => "语气正式礼貌，保持适当距离感",
            "humorous" => "语气幽默风趣，适当调侃",
            _ => "语气自然得体",
        };

        let length_guide = match length {
            "short" => "回复尽量简短，8-20字",
            "short_to_medium" => "回复短到中等，10-45字",
            "medium" => "回复中等长度，20-60字",
            _ => "回复长度自然",
        };

        let emoji_guide = if emoji_level < 0.2 {
            "尽量不用 emoji"
        } else if emoji_level < 0.5 {
            "可少量使用 emoji"
        } else {
            "可适度使用 emoji"
        };

        let humor_guide = if humor_level < 0.2 {
            "保持认真，不用幽默"
        } else if humor_level < 0.5 {
            "可带一点轻松调侃"
        } else {
            "可适度幽默"
        };
        let now = Local::now().to_rfc3339_opts(SecondsFormat::Secs, true);

        format!(
            r#"当前来信：
{incoming_message}

当前本地时间：
- {now}

联系人与本地上下文：
{conversation_context}

风格要求：
- {tone_guide}
- {length_guide}
- {emoji_guide}
- {humor_guide}

请输出 5 条候选回复，要求：
- 覆盖：稳妥、轻松、幽默一点、温柔一点、收束一点
- intent_group 优先使用：稳妥、轻松、幽默、温柔、收束、邀约、支持
- 若来信包含明确问题，至少 2 条要直接回答问题
- 若来信偏情绪表达，至少 2 条要先接住情绪
- 不要重复
- 不要带"哈哈哈哈哈哈"这类过度表达
- 不要为了显得了解对方而硬引用无关手动资料
- 敏感资料默认不用；只有当前任务明确相关且上下文允许时才可低调使用

同时输出：
- situation：一句话说明局面、时效性和当前适合动作；如果聊天时间不可信，staleness 用 unknown 或 visible_time_only
- source_summary：说明使用了当前输入、哪类本地上下文、是否引用手动资料；不得伪造来源
- action_card：判断当前更适合继续聊、收束、轻跟进、不要推进、修复，或只是轻试探邀约；必须给置信度和原因
- memory_candidates：只提取对方明确说出的事件、偏好、禁忌、压力点或关系节点；每条带来源摘录
- memory_candidates 的 summary/value/source_quote/reason 要可供用户在收件箱里判断是否保存
- reminder_candidates：只为考试、面试、加班、出差、生病、情绪低落、生日等明确事件建议提醒；默认不超过 2 条
- reminder_candidates 要填写 kind 和 cooldown_key，同一联系人相同 cooldown_key 后续会限频
- 如果没有值得记或提醒的内容，对应数组返回空数组
- 保存/提醒由用户确认，你不要写成已经保存或已经提醒"#
        )
    }

    /// Build a prompt that classifies a user-entered note into structured contact facts.
    pub fn contact_fact_classification_prompt(&self, contact_alias: &str, note: &str) -> String {
        format!(
            r#"你是 EchoMate 的本地资料归类器。用户手动补充了联系人资料，请把自然语言归类为结构化 facts。

联系人：{contact_alias}

用户手动补充资料：
{note}

规则：
- 只输出 JSON，严格符合传入 schema。
- 所有 facts 的 fact_source 必须是 manual。
- 不要把手动资料写成聊天记录，不要生成 message。
- 只抽取资料中明确出现或可直接归一化的事实，不要扩展想象。
- 明确出生年份可归类为 birth_year；只确定年龄段或代际时用 age_band。
- 城市信息要区分 hometown/current_city/work_city；“A 市人”通常是 hometown，“在 B 市工作”是 work_city。
- 高敏信息标 sensitivity=high 或 forbidden；默认 usage_policy=never 或 rare。
- 普通身份/地域/工作地信息可设 sensitivity=normal，usage_policy=contextual。
- 短期状态设置 ttl_days；长期背景可为 null。
- usage_guidance 要说明这些 facts 只能在相关场景使用，不能每次硬引用。"#,
            contact_alias = contact_alias.trim(),
            note = note.trim(),
        )
    }

    /// Build the task prompt for a screenshot-based chat context.
    pub fn screenshot_task_prompt(
        &self,
        image_width: u32,
        image_height: u32,
        local_analysis: &ScreenshotAnalysis,
        conversation_context: &str,
        tone: &str,
        length: &str,
        emoji_level: f64,
        humor_level: f64,
    ) -> String {
        let base = self.task_prompt(
            "见随附聊天截图。请按截图中的视觉方向读取上下文：左侧气泡是对方说的话，右侧气泡是我说的话。重点理解最近几轮对话，尤其是最后一条对方消息，再生成我可以继续发送的回复。",
            conversation_context,
            tone,
            length,
            emoji_level,
            humor_level,
        );

        format!(
            r#"{base}

截图上下文：
- 图片尺寸：{image_width}x{image_height}
- 本地 OCR/解析预览：
{local_analysis_block}
- 左侧消息代表对方，右侧消息代表我
- 先结合本地 OCR、截图视觉和双方上下文判断关系、语气和当前话题
- 如果截图里有“今天/昨天/具体几点”等时间分隔，请把这个可见聊天时间用于判断消息新旧，并在 context_summary.summary 中简短保留
- 不要把 EchoMate 读取截图的时间当成截图里消息的发送时间
- 不要把截图内容逐字转写给用户
- 对图片/表情/引用消息只做占位说明，不能臆测图片内容
- screenshot_analysis 必须输出 turns、最后一条可回复消息、可见时间、staleness 和 warnings
- 如果截图中文字不完整或难以辨认，warnings 说明原因，候选回复降级为安全、轻量、可继续推进对话的回复"#,
            local_analysis_block = screenshot_analysis_prompt_block(local_analysis)
        )
    }

    /// Build a prompt for proactive topic starters that do not depend on the latest chat.
    pub fn topic_task_prompt(
        &self,
        conversation_context: &str,
        tone: &str,
        length: &str,
        emoji_level: f64,
        humor_level: f64,
        topic_hint: Option<&str>,
    ) -> String {
        let mut base = self.task_prompt(
            "用户当前不是要回复最后一条聊天记录，而是想主动找一个自然、低压的话题开启或续上聊天。请不要假设对方刚刚说了什么，也不要引用不存在的上下文。",
            conversation_context,
            tone,
            length,
            emoji_level,
            humor_level,
        );

        if let Some(hint) = topic_hint.map(str::trim).filter(|hint| !hint.is_empty()) {
            base.push_str(&format!(
                r#"

用户本次找话题参考：
- {hint}

使用规则：
- 这只是用户给 EchoMate 的方向提示，不是对方说过的话，也不是聊天记录。
- 可以围绕这个方向生成更具体、更像用户会发的开场。
- 如果它和联系人上下文无关或显得突兀，可以弱化或不用，但不要忽略用户意图。
- 不要在候选里写“你刚才说/我们刚聊到”，除非可靠来源明确支持。"#,
                hint = hint
            ));
        }

        base.push_str(
            r#"

主动找话题模式：
- 生成 5 条“用户可以主动发出去”的中文开场/续聊候选
- 不要依赖最后聊天记录，不要写成回答某个具体问题
- 必须区分“本地读取/保存时间”和“聊天发送时间”；截图/剪贴板的保存时间不能当成对方刚刚发消息
- 只有截图里可见的时间分隔、通知时间或上下文本身明确说出的时间，才可用于判断聊天新旧
- 如果引用几小时前或昨天的话题，必须显式写成“下午/昨天/之前聊到...”的轻提起；如果没有可靠聊天时间或这样显得突兀，就换成新的低压话题
- 不要用“我也想吃/那现在...”这类即时接话去承接时间不明或已经过时的话题
- 话题要轻、自然、可接可不接，避免查户口、质问或高压邀约
- 优先覆盖：日常轻分享、低压关心、共同兴趣探口风、轻松吐槽、自然邀约铺垫
- 如果缺少对方偏好，就使用通用但不尴尬的话题
- action_card 请选择 light_follow_up 或 continue_chat，reason 说明这是主动低压开启话题
- memory_candidates 和 reminder_candidates 默认返回空数组，除非提示里明确出现值得记的事实"#,
        );
        base
    }
}

fn screenshot_analysis_prompt_block(analysis: &ScreenshotAnalysis) -> String {
    if analysis.turns.is_empty() {
        return format!(
            "- 本地 OCR 未得到可用 turn；warnings：{}",
            if analysis.warnings.is_empty() {
                "无".to_string()
            } else {
                analysis.warnings.join("；")
            }
        );
    }
    let mut lines = Vec::new();
    for turn in analysis.turns.iter().take(12) {
        lines.push(format!(
            "- {} / {} / {}：{}",
            turn.speaker,
            turn.media_kind,
            if turn.visible_time_label.is_empty() {
                "时间未知"
            } else {
                &turn.visible_time_label
            },
            turn.text
        ));
    }
    lines.push(format!(
        "- 最后一条可回复消息：{}",
        if analysis.last_reply_target.is_empty() {
            "未确定"
        } else {
            &analysis.last_reply_target
        }
    ));
    lines.push(format!(
        "- 时间可信度：{} / {}",
        analysis.inferred_chat_time, analysis.staleness
    ));
    if !analysis.warnings.is_empty() {
        lines.push(format!("- warnings：{}", analysis.warnings.join("；")));
    }
    lines.join("\n")
}
