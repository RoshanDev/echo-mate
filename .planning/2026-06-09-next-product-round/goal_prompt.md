# 新对话 Goal 提示词

请创建一个 goal 并开始实施 EchoMate 下一轮 Phase 1 的核心功能：上下文完整性与来源追踪，同时把“联系人手动补充资料”作为第一批 schema 设计一起落地。

工作目录：`/Users/roshan/Developer/rust/echo-mate`

先阅读这些文件：

- `AGENTS.md`
- `.planning/.active_plan`
- `.planning/2026-06-09-next-product-round/task_plan.md`
- `.planning/2026-06-09-next-product-round/findings.md`
- `.planning/2026-06-09-next-product-round/progress.md`
- `docs/deep-research-report-EchoMate 深度研究与产品扩展方案-2026-06-09.md`

目标：

实现 EchoMate 的上下文可靠性底座，让每次生成都能解释“用了什么上下文、来源是什么、时间是否可信、是否来自用户手动补充资料”。同时增加联系人资料补充能力的基础设计：用户可以录入聊天记录之外的信息，例如“联系人A 90 后、A 市人、在 B 市工作”，EchoMate 需要调用 Claude Code 或 Codex CLI 将其归类为结构化事实，再由本地策略决定后续什么时候放进生成 prompt。

必须遵守：

- 不自动发送消息。
- 不做隐蔽监听，不扫描全量聊天历史。
- 不把 e2e/mock/test 数据写入真实用户上下文。
- 不把手动补充资料伪装成聊天记录。
- 不把截图捕获时间当作真实聊天时间。
- 不让 provider 自带测试语料污染真实联系人。
- 引用任何记忆或手动资料时，都要能显示来源。

第一批实现范围：

1. 梳理现有 DB schema、Tauri commands、provider 调用、上下文摘要、记忆/提醒、设置页和主弹窗数据流。
2. 设计 source/provenance schema：至少覆盖截图/剪贴板、聊天摘要、已批准记忆、候选记忆、提醒上下文、用户手动补充资料。
3. 设计 `contact_facts` 或同等结构化事实表，字段至少包括：`contact_id`、`fact_type`、`value`、`normalized_value`、`source_note`、`fact_source`、`sensitivity`、`confidence`、`ttl_days`、`usage_policy`、`created_at`、`updated_at`、`last_used_at`。
4. 设计并实现手动资料归类 prompt / provider schema：输入自然语言资料，输出结构化 facts。示例输入：“联系人A 90 后，A 市人，在 B 市工作”。示例输出应能区分出生年份/年龄段、籍贯、工作城市，并标记来源为 `manual`。
5. 设计引用策略：生成回复、找话题、提醒、关系卡分别如何决定是否使用某条 fact。无关资料不得硬引用；敏感资料默认不用；引用时 UI 或 source card 要说明“用户手动补充”。
6. 改进 provider JSON 解析错误：空输出、非 JSON、schema drift、stderr 都要给出可诊断信息。
7. 补回归测试：清除联系人上下文后不能引用旧消息、旧记忆、旧手动资料；e2e/mock 不能污染真实 DB；手动 fact 不进入消息表。

工作方式：

- 先读代码和计划，不要直接大改。
- 找到现有模式后给出简短实施计划，再开始改。
- 使用 `rg` 搜索，使用 `apply_patch` 手动编辑。
- 不要回滚用户未要求回滚的改动。
- 每完成一个小阶段更新 `.planning/2026-06-09-next-product-round/progress.md`。
- 修改后运行能跑的测试；至少运行相关 Rust/前端测试或说明无法运行的原因。
- 最后给出改动摘要、测试结果、未完成项。
