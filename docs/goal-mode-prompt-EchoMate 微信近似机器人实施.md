# EchoMate Goal 模式提示词：微信近似机器人实施

用途：把下面的提示词粘贴到新会话的 Goal 模式 / 长期执行模式里，让它基于当前仓库、规划文件和微信机器人集成可行性报告，直接实施 EchoMate 的“微信近似机器人”路线。

核心建议：正式版不接微信机器人，不自动发消息。先做联系人 allowlist、本地上下文、采用回写、权限透明、Windows 近似入站提醒、macOS 上下文近似辅助，并把所有能力并入现有候选回复、记忆和提醒链路。

## 可复制提示词

```text
$planning-with-files-zh

你是 EchoMate 项目的 autonomous product-engineering agent。请进入 Goal 模式，持续工作到目标被真实完成并验证，不要只输出计划。你必须使用文件规划系统管理上下文：先读取或创建 task_plan.md、findings.md、progress.md；每个阶段完成后更新这些文件；发生错误必须记录；不要重复同样的失败操作。

项目路径：/home/roshan/Developer/echo-mate

必须先读取的项目文件：
- AGENTS.md（如果仓库根目录没有，就遵守本会话注入的 AGENTS 指令）
- task_plan.md
- findings.md
- progress.md
- docs/deep-research-report-EchoMate 与微信机器人集成可行性报告.md
- docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md

当前计划状态：
- Phase H 已完成：微信机器人集成可行性规划已落盘。
- 你要从 Phase W0 开始实施。
- Phase W7 “实验性 weixin-bot sidecar”是 gated，不要默认实施。只有用户明确批准 sidecar 和新增 Node 依赖时才进入 W7。

本轮 Goal：
实现 EchoMate 的“微信近似机器人”能力，完成 task_plan.md 中 Phase W0-W6。

一句话目标：
不碰微信协议、不自动发送、不抓全量历史，在现有 Tauri/Rust/vanilla JS 应用中加入联系人白名单、联系人上下文、采用回复回写、透明权限说明、平台近似信号和上下文合并，让 EchoMate 更像“本地关系 CRM + AI 回复副驾”。

必须坚持的产品边界：
- 绝不自动发送微信消息。
- 绝不主动起聊。
- 绝不做全量聊天历史抓取。
- 绝不默认接入微信机器人、OpenClaw、weixin-bot 或任何二维码登录 sidecar。
- 绝不做群监控。
- 绝不自动保存敏感信息。
- 绝不输出 PUA、操控、冷暴力、情绪打压或伪确定评分。
- 所有保存、提醒、上下文记录都必须有用户可见的来源、删除入口和关闭路径。

已实现能力：
- Tauri 2 + Rust 后端 + vanilla HTML/CSS/JS 前端。
- 全局热键、剪贴板文本、选中文本热键、截图上下文。
- Codex / Claude CLI Provider。
- 5 条候选回复。
- next_action、memory_candidates、reminder_candidates、context_summary。
- SQLite 本地表：memory_item、reminder、context_summary、reply_feedback。
- 本地提醒到点通知、提醒面板恢复、跟进候选。
- “找话题”模式。
- Windows release E2E runner 覆盖文本、选中文本、截图、记忆、提醒、topic。

关键参考结论：
- Tencent openclaw-weixin / iLink / weixin-bot 技术上可做入站 bot，但不适合作为 EchoMate 正式主线。
- iLink 发送依赖 context_token；主动新会话受限，产品风险高。
- Windows Notification Listener 可读通知但需要用户授权和 capability，权限敏感。
- macOS 没有确认存在与 Windows Notification Listener 同等级的公开跨 app 通知读取能力；macOS 只做前台 app、Pasteboard、Accessibility 选中文本/窗口标题等近似上下文能力。
- 报告里的引用标记不能当实施证据；实施前重新打开主源确认当前 API、包名、许可、兼容状态。

实施阶段：

Phase W0：实施前复核与范围锁定
- 读取 task_plan.md / findings.md / progress.md，确认 Phase H complete。
- 重新核验关键外部主源当前状态：
  - https://github.com/Tencent/openclaw-weixin
  - https://github.com/epiral/weixin-bot/blob/main/docs/protocol-spec.md
  - https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/notification-listener
  - https://developer.apple.com/documentation/appkit/nsworkspace/didactivateapplicationnotification
  - https://developer.apple.com/documentation/appkit/nspasteboard/changecount
  - https://developer.apple.com/documentation/applicationservices/axuielement_h
  - https://developer.apple.com/documentation/applicationservices/kaxselectedtextattribute
- 在 findings.md 记录复核结果。
- 明确本轮只做近似机器人，不做 sidecar。
- 如果 Windows Notification Listener 在当前 Tauri/打包形态下不可直接实现，不要伪装完成；记录原因并实现安全降级路径。

Phase W1：联系人 allowlist 与产品边界
- 增加联系人模型：contacts(id, alias, channel, is_allowlisted, created_at, updated_at)。
- 增加联系人设置 UI：别名、渠道、启用/停用、删除。
- 所有入站提醒、上下文缓存、记忆抽取仅对白名单联系人启用。
- UI 文案明确：EchoMate 只生成候选并复制，不自动发送。
- 增加权限说明区域：读取什么、为什么读、如何关闭、如何删除数据。

Phase W2：本地联系人上下文与采用回写
- 复用现有 memory_item、reminder、context_summary、reply_feedback，不重建记忆系统。
- 补齐 messages / recent timeline：
  - messages(id, contact_id, role, text, source, approved, created_at)
  - source 可为 clipboard、screenshot、notification、manual、topic。
- 补齐 style_profile 持久化：
  - style_profile(id, profile_json, sample_count, updated_at)
  - 存摘要，不存无限原始样本。
- 将用户复制/采用的候选回写为风格样本和 generation log。
- 增加保留期限、联系人级清空、全局隐私模式。

Phase W3：Windows 近似入站提醒 helper
- 目标是可选能力，不是默认后台监听。
- 优先验证 Windows Notification Listener 在当前应用形态是否可用。
- 如果可用：实现显式权限请求、权限状态展示、只读微信通知、按 allowlist 触发本地 signal。
- 如果不可用：实现前台窗口/剪贴板相关性 fallback，并把 Notification Listener 留作打包/权限后续项。
- 禁止清理、删除或转发用户系统通知。
- 权限被撤回时静默降级到热键/截图主流程。
- 记录误报/漏报日志，并提供关闭路径。

Phase W4：macOS 上下文近似 helper
- 不承诺后台实时读取微信通知。
- 使用前台应用变化、Pasteboard、Accessibility 选中文本/窗口标题做上下文辅助。
- 所有 Accessibility 能力必须用户主动开启，并支持一键关闭。
- 失败时回退到现有复制/截图/热键路径。
- 不因 Accessibility 失败阻断核心候选回复功能。

Phase W5：生成链路与弹窗整合
- 将联系人记忆、最近上下文、风格画像、当前信号合并进现有 PromptComposer。
- 保留 5 条候选回复。
- 继续输出 next_action、memory_candidates、reminder_candidates、context_summary。
- UI 增加：
  - 为什么提醒我
  - 上下文来源
  - 删除这条上下文
  - 联系人不在 allowlist 时不保存
- 针对低置信、敏感信息、联系人不在 allowlist 的场景默认不建议保存和提醒。

Phase W6：验证与隐私回归
- 单元测试：
  - schema
  - parser
  - repository
  - retention
  - allowlist
  - 权限关闭降级
- 前端 harness：
  - 联系人设置
  - 来源说明
  - 删除上下文
  - 禁止自动发送断言
- Windows E2E：
  - 通知 helper 或 fallback
  - 热键
  - 截图
  - 上下文合并
  - 提醒恢复
- macOS 手动验证：
  - Accessibility 开关
  - 选中文本读取失败回退
  - Pasteboard 路径

工程约束：
- 先读代码再改代码。
- 保持现有 Tauri/Rust/vanilla JS 架构。
- 不新增依赖，除非用户明确批准；如确实需要，先在 task_plan.md 和 findings.md 写清原因、替代方案和风险。
- 优先复用 Orchestrator、PromptComposer、schema、parser、commands、MemoryRepository、migrations、现有前端组件。
- Provider 输出 schema 必须和 Codex strict structured output 兼容：additionalProperties=false 时 properties 与 required 对齐。
- Windows/WSL Provider 调用不能回退出黑窗口、卡死或旧 Node shebang 问题。
- UI 要紧凑、低压、工具化，不做营销页，不做浮夸说明。

验证命令：
- cargo fmt --manifest-path src-tauri/Cargo.toml --check
- cargo test --manifest-path src-tauri/Cargo.toml
- cargo check --manifest-path src-tauri/Cargo.toml
- cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu
- node --check frontend/main.js
- node --check tests/windows-e2e-runner.mjs
- chromium-browser --headless=new --disable-gpu --no-sandbox --allow-file-access-from-files --virtual-time-budget=5000 --dump-dom tests/frontend-memory-reminder-harness.html
- git diff --check
- make win-run
- Windows E2E runner，必要时用 ECHOMATE_E2E_MOCK_PROVIDER=1 和 WebView2 CDP。

完成标准：
- 用户能创建、编辑、禁用、删除联系人 allowlist 项。
- 文本、截图、找话题仍能生成 5 条候选回复。
- 生成时能合并联系人记忆、最近上下文和风格画像。
- 用户复制/采用的候选会形成可控的本地风格样本。
- 入站/近似信号只对白名单联系人触发，不自动生成和发送。
- UI 能显示“为什么提醒我 / 上下文来源 / 删除入口”。
- 所有上下文和记忆都有删除路径。
- 权限关闭或平台能力不可用时，核心热键/截图流程仍可工作。
- 所有验证命令通过；如果某个平台手动验证无法执行，必须明确写入 progress.md 的 Not-tested。
- task_plan.md 标记 W0-W6 状态，progress.md 记录完整验证证据，findings.md 记录风险和后续 gated W7。

最终输出：
- 简短说明完成了哪些能力。
- 列出关键修改文件。
- 列出验证命令和结果。
- 列出剩余风险。
- 明确说明没有实现自动代发、主动起聊、全量历史抓取、群监控、默认 sidecar。
```

## 新会话使用方式

1. 新会话打开 `/home/roshan/Developer/echo-mate`。
2. 直接粘贴“可复制提示词”整段。
3. 让新会话从 Phase W0 开始执行。

## 范围提醒

第一轮完成 W0-W6 就够了。W7 `weixin-bot` sidecar 是实验功能，不要让新会话默认开始；它会引入二维码登录、凭证保存和新依赖，必须单独批准。
