# EchoMate Goal 模式提示词：事件型记忆提醒 MVP

用途：把下面的提示词粘贴到 ChatGPT / Codex / Claude 的 Goal 模式或长期执行模式里，让它基于现有仓库和 Deep Research 报告，持续规划并实现 EchoMate 的下一阶段产品扩展。

核心定位：EchoMate 不只是“截图生成回复器”，而是“个人关系 CRM + AI 回复副驾”。下一阶段优先做“事件型记忆提醒 MVP”：记住她明确说过的重要事，在合适时间提醒用户，并给出低压、自然、尊重边界的 follow-up 候选。

## 可复制提示词

```text
$planning-with-files-zh

你是 EchoMate 项目的 autonomous product-engineering agent。请进入 Goal 模式，持续工作到目标被真实完成并验证，不要只输出计划。你必须使用文件规划系统管理上下文：先读取或创建 task_plan.md、findings.md、progress.md；每个阶段完成后更新这些文件；发生错误必须记录；不要重复同样的失败操作。

项目路径：/home/roshan/Developer/echo-mate
核心参考报告：docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md

当前已实现能力：
- Tauri 2 + Rust 后端 + vanilla HTML/CSS/JS 前端。
- Windows 桌面程序，支持全局快捷键触发。
- 支持读取剪贴板文本，也支持按热键自动复制当前选中文本。
- 支持“截图上下文”：用户框选聊天记录截图，左侧气泡视为对方，右侧气泡视为我，走 Codex 图片输入生成候选回复。
- 支持 Claude / Codex 本地 CLI Provider，经 Windows 调用 WSL2。
- 弹窗展示 5 条候选回复，一键复制。
- 有基础风格设置：温和、正式、幽默、长度、emoji 程度等。
- 已处理 Windows 黑窗口、进程超时、候选列表滚动、复制候选崩溃、Codex WSL Node 版本、Codex strict schema 等问题。

产品目标：
把 EchoMate 从“一次性回复生成器”扩展成“个人关系 CRM + AI 回复副驾”。重点不是 PUA、操控、骚扰或自动代聊，而是帮助用户更有记性、更有分寸、更自然地维持联系。最终目标是提升用户和喜欢的人持续聊天、自然推进关系的能力，但必须以真实表达、尊重边界、低打扰、用户确认为前提。

本轮 Goal 的最高优先级：
实现“事件型记忆提醒 MVP”。

MVP 定义：
用户触发快捷键或截图上下文后，EchoMate 在生成 5 条候选回复的同时，识别明确事件、偏好、禁忌和跟进机会，显示为可确认的记忆/提醒卡片。用户确认后，本地保存记忆或提醒；到点后发送本地通知；用户点击通知后看到来源、当前建议和 3 条跟进候选。

必须先做的规划工作：
1. 读取 AGENTS.md、task_plan.md、findings.md、progress.md。
2. 读取 docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md，重点提炼：
   - “个人关系 CRM + AI 回复副驾”定位。
   - 三阶段路线。
   - “定时关怀、历史提醒与记忆系统设计”。
   - “最高优先级 MVP：事件型记忆提醒 MVP”。
   - 不应该做的功能和安全边界。
3. 在 task_plan.md 中新增本轮实施阶段，至少包含：
   - Phase A：现有代码与数据层调研。
   - Phase B：事件/记忆/提醒 schema 设计。
   - Phase C：Provider 输出 schema 与 prompt 扩展。
   - Phase D：弹窗 UI 增加“当前建议 / 可能值得记住 / 提醒建议”。
   - Phase E：本地存储与用户确认流程。
   - Phase F：提醒调度与本地通知。
   - Phase G：测试、Windows 构建、手动 E2E。
4. 在 findings.md 中记录报告提炼出的关键产品约束。
5. 在 progress.md 中持续记录每次实现、验证、错误和修复。

实现范围：
1. 下一行动建议
   - 在候选回复之外输出一个 action card。
   - 类型包括：continue_chat、wrap_up、light_follow_up、do_not_push、safe_repair、soft_invite_candidate。
   - 必须带原因和置信度，避免强断言。

2. 记忆候选
   - 从文本或截图上下文中提取明确、值得记的事实。
   - 类型包括：event、preference、boundary、stress_point、relationship_milestone。
   - 每条记忆必须有 source_kind、source_ref 或 source_excerpt。
   - 默认只是候选，必须用户确认后保存。
   - 敏感信息要分级：普通 / 中 / 高 / 禁止。

3. 提醒候选
   - 从明确事件中提取提醒建议，例如考试、面试、加班、出差、生病、情绪低落、生日等。
   - 每条提醒必须有 recommended_time、reason、suggested_follow_up。
   - 默认最多一到两次提醒，不做高频骚扰。
   - 用户必须能确认、改时间、忽略。

4. 本地存储
   - 优先复用现有 store/memory 模块和 SQLite/rusqlite。
   - 设计最小数据结构：
     memory_item(id,type,value,source_kind,source_ref,source_excerpt,confidence,sensitivity,expires_at,status,created_at,updated_at)
     reminder(id,memory_id,trigger_at,reason,suggested_follow_up,status,snooze_count,created_at,updated_at)
     context_summary(id,source_kind,source_ref,summary,created_at)
     reply_feedback(id,generation_id,action,candidate_index,created_at)
   - 不要自动导入历史聊天，不要偷偷扫描微信数据库，不要常驻监听全部剪贴板。

5. UI
   - 在现有弹窗中增加紧凑区域，不要做营销页。
   - 顶部或候选区前增加三块：
     当前适合做什么
     可能值得记住
     提醒建议
   - 所有卡片必须可忽略；保存/提醒必须是用户显式点击。
   - 文案要低压，不要油腻，不要操控。

6. 通知与提醒
   - 使用 Tauri 本地通知能力或仓库已有可行替代方案。
   - 到点通知只提醒用户，不自动发送消息。
   - 通知点击后打开 EchoMate 小面板，显示来源和跟进候选。
   - 加 quiet hours / 冷却时间 / 近期已联系则抑制 的最小规则；如果实现成本较高，先在计划里拆成后续阶段，但不要破坏数据结构。

安全边界：
- 绝不自动发送消息。
- 绝不输出 PUA、控制、冷暴力、情绪操控话术。
- 不做“她对你兴趣值 83 分”之类伪确定评分。
- 不自动推断生理期、病史、住址、定位规律、家庭矛盾等高敏信息。
- 不把“慢回”直接判断为没兴趣；所有关系阶段和情绪判断必须低置信、可解释、可反驳。
- 每条记忆必须支持来源回显和删除。
- 默认本地优先、数据最小化、用户确认。

工程约束：
- 先读代码再改代码，保持现有 Tauri/Rust/vanilla JS 架构。
- 不要引入大依赖，除非确实必要并在 task_plan.md 记录原因。
- diffs 要小、可回退、可审查。
- 优先复用现有 Orchestrator、PromptComposer、schema、parser、commands、store、memory 模块。
- Provider 输出 schema 必须和 Codex strict structured output 兼容：additionalProperties=false 时，properties 与 required 要保持一致。
- Windows/WSL 调用路径要继续避免黑窗口、卡死和旧 Node shebang 问题。

验证要求：
- cargo fmt
- cargo test --manifest-path src-tauri/Cargo.toml
- cargo check --manifest-path src-tauri/Cargo.toml
- cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu
- node --check frontend/main.js
- make win-run
- 手动 E2E：文本触发、选中文本热键、截图上下文、保存记忆、创建提醒、通知打开。

完成标准：
- 用户触发文本或截图生成时，仍能得到 5 条候选回复。
- UI 能展示“当前适合做什么 / 可能值得记住 / 提醒建议”。
- 用户能确认保存一条记忆，能创建一条提醒，能忽略候选。
- 到点提醒能以本地通知出现。
- 通知或提醒入口能回到跟进建议。
- 所有新增记忆/提醒都有来源和删除/忽略路径。
- 所有测试通过，Windows release 能启动。
- progress.md 记录完整验证结果，task_plan.md 标记阶段状态。

最终输出：
- 简短说明完成了哪些能力。
- 列出关键修改文件。
- 列出验证命令和结果。
- 列出剩余风险和下一阶段建议。
```

## 使用建议

如果 Goal 模式支持“目标描述”和“初始指令”分栏，可以这样拆：

目标描述：

```text
基于 EchoMate 现有 Tauri 桌面应用和 docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md，实现事件型记忆提醒 MVP：在生成候选回复的同时提取记忆/提醒候选，用户确认后本地保存，到点本地通知，并给出跟进候选。必须使用 $planning-with-files-zh 管理计划和进度。
```

初始指令：粘贴上方“可复制提示词”的完整内容。

## 范围控制

第一轮不要追求完整关系 CRM。优先完成一条闭环：

1. 从当前文本/截图上下文中识别一个明确事件。
2. 展示提醒候选。
3. 用户确认。
4. 本地保存。
5. 到点通知。
6. 打开后给出跟进候选。

只要这条链路跑通，EchoMate 就从“回复器”迈进了“关系记忆提醒副驾”。
