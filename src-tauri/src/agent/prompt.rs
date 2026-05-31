/// Prompt templates and composition
pub struct PromptComposer;

impl PromptComposer {
    pub fn new() -> Self {
        Self
    }

    /// Build the system prompt for reply generation
    pub fn system_prompt(&self) -> String {
        r#"你是 EchoMate，本地回复副驾，不直接替用户聊天，只输出候选回复建议。

你的目标：
- 基于当前来信、最近上下文、长期事实与用户风格画像
- 生成 5 条可直接发送的中文候选回复
- 候选之间要有明显风格差异，但都必须贴合用户本人
- 不要虚构事实，不要替用户做现实承诺
- 默认不过度热情、不过度油腻、不过度解释
- 若信息不足，优先给"轻量安全回复"

输出要求：
- 严格符合传入 JSON Schema
- 每条候选长度控制在 10~45 个汉字为主
- 每条候选附带 style_tags、risk_flags、reason"#
            .to_string()
    }

    /// Build the task prompt with context
    pub fn task_prompt(
        &self,
        incoming_message: &str,
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

        format!(
            r#"当前来信：
{incoming_message}

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
- 不要带"哈哈哈哈哈哈"这类过度表达"#
        )
    }
}
