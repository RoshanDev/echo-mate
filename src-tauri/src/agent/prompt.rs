/// Prompt templates and composition
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
- 默认不过度热情、不过度油腻、不过度解释
- 若信息不足，优先给"轻量安全回复"

安全边界：
- 绝不自动发送消息，只给用户可复制的建议
- 不输出 PUA、控制、冷暴力、情绪操控话术
- 不做"兴趣值 83 分"这类伪确定评分
- 不把慢回直接判断为没兴趣；关系/情绪判断必须低置信、给理由
- 不自动推断生理期、病史、住址、定位规律、家庭矛盾等高敏信息
- 记忆/提醒只从明确说过的事件、偏好、禁忌、压力点中提取

输出要求：
- 严格符合传入 JSON Schema
- 每条候选长度控制在 10~45 个汉字为主
- 每条候选附带 style_tags、risk_flags、reason
- action_card 必须选择 schema 中的 action_type，reason 不能强断言
- memory_candidates 默认 0-3 条，只放明确、值得记的事实；敏感或不该记的信息标 forbidden
- reminder_candidates 默认 0-2 条，只针对明确事件；trigger_at 尽量用 RFC3339
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
- 若来信包含明确问题，至少 2 条要直接回答问题
- 若来信偏情绪表达，至少 2 条要先接住情绪
- 不要重复
- 不要带"哈哈哈哈哈哈"这类过度表达

同时输出：
- action_card：判断当前更适合继续聊、收束、轻跟进、不要推进、修复，或只是轻试探邀约；必须给置信度和原因
- memory_candidates：只提取对方明确说出的事件、偏好、禁忌、压力点或关系节点；每条带来源摘录
- reminder_candidates：只为考试、面试、加班、出差、生病、情绪低落、生日等明确事件建议提醒；默认不超过 2 条
- 如果没有值得记或提醒的内容，对应数组返回空数组
- 保存/提醒由用户确认，你不要写成已经保存或已经提醒"#
        )
    }

    /// Build the task prompt for a screenshot-based chat context.
    pub fn screenshot_task_prompt(
        &self,
        image_width: u32,
        image_height: u32,
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
- 左侧消息代表对方，右侧消息代表我
- 先结合双方上下文判断关系、语气和当前话题
- 如果截图里有“今天/昨天/具体几点”等时间分隔，请把这个可见聊天时间用于判断消息新旧，并在 context_summary.summary 中简短保留
- 不要把 EchoMate 读取截图的时间当成截图里消息的发送时间
- 不要把截图内容逐字转写给用户
- 如果截图中文字不完整或难以辨认，生成安全、轻量、可继续推进对话的回复"#
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
    ) -> String {
        let mut base = self.task_prompt(
            "用户当前不是要回复最后一条聊天记录，而是想主动找一个自然、低压的话题开启或续上聊天。请不要假设对方刚刚说了什么，也不要引用不存在的上下文。",
            conversation_context,
            tone,
            length,
            emoji_level,
            humor_level,
        );

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
