# Task Plan: EchoMate MVP 发现版 (Discovery Phase)

## Goal
实现 EchoMate 发现版 MVP：用户复制微信消息 → 按全局热键 → 读取剪贴板 → 调用 Codex/Claude CLI → 弹出 5 条候选回复 → 一键复制。

## Current Phase
Phase W6 complete: 微信近似机器人 W0-W6 已实现并验证；Windows 完整桌面 E2E 已通过，macOS 手动验证在当前 Linux/Windows 环境中记录为 Not-tested；W7 sidecar 仍为 gated，未实施。

## Phases

### Phase 1: 前端 UI 改造
- [x] 候选人弹窗 UI（匹配设计稿：标题栏、5条候选卡片、复制按钮、底部操作栏）
- [x] 设置页 UI（热键配置、Provider 选择、隐私开关、风格画像）
- [x] 前端状态管理与 Tauri invoke 桥接
- **Status:** complete

### Phase 2: 平台层实现
- [x] 全局热键注册/注销（tauri-plugin-global-shortcut）
- [x] 剪贴板读写（tauri-plugin-clipboard-manager）
- [x] 系统托盘与窗口管理（显示/隐藏弹窗、置顶、定位）
- **Status:** complete

### Phase 3: Provider 适配器
- [x] Claude CLI 适配器（claude -p + JSON Schema）
- [x] Codex CLI 适配器（codex exec + output-schema）
- [x] 超时控制、进程隔离、错误处理
- **Status:** complete

### Phase 4: Agent 编排层
- [x] Orchestrator 主流程（clipboard → prompt → provider → candidates）
- [x] Prompt 组装（模板渲染 + 上下文注入）
- [x] JSON Schema 验证与输出解析
- **Status:** complete

### Phase 5: 集成联调
- [x] Tauri commands 对接前端
- [x] 事件流（热键触发 → 后端处理 → 前端展示）
- [x] 端到端流程验证
- **Status:** complete

### Phase 6: 测试与文档
- [x] cargo check 通过（零错误零警告）
- [x] 手动 E2E 验证（macOS）— 需用户在本地执行 `npx tauri dev`
- [x] 更新 SETUP.md 与使用说明
- **Status:** complete

### Phase 7: Goal 模式产品扩展提示词
- [x] 读取 `$planning-with-files-zh` 工作流要求
- [x] 读取 Deep Research 产品扩展报告
- [x] 提炼“事件型记忆提醒 MVP”实施目标、边界和验证标准
- [x] 写入 Goal 模式提示词文档
- **Status:** complete

### Phase A: 事件型记忆提醒 MVP 现有代码与数据层调研
- [x] 读取 AGENTS.md、task_plan.md、findings.md、progress.md
- [x] 读取 Deep Research 产品扩展报告并提炼约束
- [x] 调研 Orchestrator、PromptComposer、schema、parser、commands、store、memory 模块
- [x] 明确现有测试与验证命令覆盖面
- **Status:** complete

### Phase B: 事件/记忆/提醒 schema 设计
- [x] 设计 next_action、memory_candidates、reminder_candidates 的 provider 输出结构
- [x] 设计 memory_item、reminder、context_summary、reply_feedback 的本地存储结构
- [x] 保持 Codex strict structured output 兼容
- **Status:** complete

### Phase C: Provider 输出 schema 与 prompt 扩展
- [x] 扩展 prompt，要求低压、可解释、低置信、用户确认
- [x] 扩展 JSON Schema，确保 additionalProperties=false 与 required 对齐
- [x] 扩展 parser/domain 类型，保留 5 条候选回复
- **Status:** complete

### Phase D: 弹窗 UI 增加产品卡片
- [x] 增加“当前适合做什么”
- [x] 增加“可能值得记住”
- [x] 增加“提醒建议”
- [x] 所有卡片支持忽略，保存/提醒必须显式点击
- **Status:** complete

### Phase E: 本地存储与用户确认流程
- [x] 实现确认保存记忆
- [x] 实现创建提醒和忽略候选
- [x] 每条记忆/提醒保留来源，提供删除/忽略路径
- **Status:** complete

### Phase F: 提醒调度与本地通知
- [x] 使用 Tauri 通知或仓库可行替代方案到点提醒
- [x] 通知/提醒到点后显示来源、当前建议和 3 条跟进候选
- [x] 加入最小 quiet hours / 冷却 / 抑制设计；高成本项记录后续阶段
- **Status:** complete

### Phase G: 测试、Windows 构建、手动 E2E
- [x] cargo fmt
- [x] cargo test --manifest-path src-tauri/Cargo.toml
- [x] cargo check --manifest-path src-tauri/Cargo.toml
- [x] cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu
- [x] node --check frontend/main.js
- [x] node --check tests/windows-e2e-runner.mjs
- [x] Chromium headless 前端闭环验证：卡片展示、保存记忆、创建提醒、提醒面板、复制跟进
- [x] make win-run
- [x] Windows E2E runner：文本触发、配置热键选中文本、截图上下文、保存记忆、创建提醒、到点提醒面板恢复
- **Status:** complete

### Phase H: 微信机器人集成可行性规划
- [x] 读取 `$planning-with-files-zh` 工作流要求并恢复现有计划上下文
- [x] 读取 `docs/deep-research-report-EchoMate 与微信机器人集成可行性报告.md`
- [x] 对关键外部技术路径做主源抽样核验
- [x] 对照现有本地记忆、提醒、截图、复制、topic 生成能力，拆分可执行路线
- [x] 更新 `task_plan.md`、`findings.md`、`progress.md`
- **Status:** complete

### Phase I: 微信近似机器人 Goal 模式实施提示词
- [x] 读取既有 Goal 模式提示词格式
- [x] 将 Phase W0-W6 转写为新会话可直接执行的 Goal 模式提示词
- [x] 明确 W7 `weixin-bot` sidecar 为 gated，不默认实施
- [x] 写入 `docs/goal-mode-prompt-EchoMate 微信近似机器人实施.md`
- [x] 更新 `task_plan.md`、`findings.md`、`progress.md`
- **Status:** complete

### Phase W0: 实施前复核与范围锁定
- [x] 复核当前 OpenClaw / iLink / `weixin-bot` / Windows Notification Listener / macOS Accessibility 文档状态
- [x] 确认本轮实施只做“近似机器人”，不做自动代发、主动起聊、全量历史抓取、群监控
- [x] 明确 Windows-first 还是双平台同步；默认 Windows-first，macOS 做上下文近似能力
- [x] 明确所有新增权限提示、数据保留、删除入口和实验功能开关
- **Status:** complete

### Phase W1: 联系人 allowlist 与产品边界
- [x] 增加联系人别名 / channel / allowlist 的本地模型与设置入口
- [x] 所有入站提醒、上下文缓存、记忆抽取仅对白名单联系人启用
- [x] UI 明确“只生成候选并复制，不自动发送”
- [x] 为权限请求提供本地解释页：读取什么、为什么读、如何关闭、如何删除
- **Status:** complete

### Phase W2: 本地联系人上下文与采用回写
- [x] 复用现有 `memory_item`、`reminder`、`context_summary`、`reply_feedback`，避免重写记忆系统
- [x] 补齐 `contacts`、最近 `messages` / timeline、`style_profile` 持久化缺口
- [x] 将用户最终复制/采用的候选回写为风格样本，只保存摘要和必要元数据
- [x] 增加保留期限、联系人级清空、全局隐私模式
- **Status:** complete

### Phase W3: Windows 近似入站提醒 helper
- [x] Spike Windows Notification Listener / 当前前台窗口 / 剪贴板关联的最小 helper
- [x] 只把微信通知转成“某联系人来了消息，要不要生成候选”的本地信号
- [x] 不清理、不删除、不转发系统通知；权限被撤回时静默降级到热键/截图主流程
- [x] 建立白名单 signal 判定日志与人工关闭路径
- **Status:** complete

### Phase W4: macOS 上下文近似 helper
- [x] 使用前台应用变化、Pasteboard、Accessibility 选中文本/窗口标题做上下文辅助
- [x] 不承诺后台实时读取微信通知
- [x] 所有 Accessibility 能力必须用户主动开启，并支持一键关闭
- [x] 失败时回退到现有复制/截图/热键路径
- **Status:** complete

### Phase W5: 生成链路与弹窗整合
- [x] 组装联系人记忆、最近上下文、风格画像、当前信号进入现有 PromptComposer
- [x] 保留 5 条候选回复，继续输出 next_action、记忆候选、提醒候选
- [x] UI 增加“为什么提醒我 / 上下文来源 / 删除这条上下文”
- [x] 针对低置信、敏感信息、联系人不在 allowlist 的场景默认不建议保存和提醒
- **Status:** complete

### Phase W6: 验证与隐私回归
- [x] 单元测试：schema、parser、repository、retention、allowlist、权限关闭降级
- [x] 前端 harness：联系人设置、来源说明、删除上下文、禁止自动发送断言
- [x] Windows E2E：通知 helper/fallback、热键、截图、上下文合并、提醒恢复
- [x] macOS 手动验证：Accessibility 开关、选中文本读取失败回退、Pasteboard 路径（当前 Linux/Windows 环境无法执行，已在 `progress.md` 记录 Not-tested）
- [x] `cargo fmt`、`cargo test`、`cargo check`、Windows target check、`node --check`、`git diff --check`
- **Status:** complete

### Phase W7: 实验性 weixin-bot sidecar（显式批准后才做）
- [ ] 单独 Node sidecar，不嵌入正式主进程，不默认启动
- [ ] 二维码登录、凭证本地保存、凭证删除、重登录失败恢复
- [ ] 只订阅入站消息并投递到 localhost；不调用 reply/send，不做主动发送
- [ ] 联系人 allowlist、实验开关、日志脱敏、卸载清理
- [ ] Windows first；macOS hardening 作为后续子阶段
- **Status:** gated

## Key Questions
1. ~~React+TypeScript+Vite or vanilla HTML/CSS/JS?~~ → 已有 vanilla HTML/CSS/JS 壳，保持轻量
2. Codex CLI 是否已安装在用户机器? → 已确认 codex CLI 可用，Claude CLI 已在 PATH
3. 热键默认值? → CmdOrCtrl+Shift+Space
4. 是否需要 tray-icon feature? → 已启用
5. 微信集成是否要做正式机器人? → 默认不做；正式主线只做“近似机器人”
6. 是否允许实验性 sidecar? → 需要显式批准；默认只规划，不实施
7. 是否 Windows-first? → 默认 Windows-first，macOS 做上下文近似和手动触发增强

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| 前端用 vanilla HTML/CSS/JS | 弹窗简单，无需框架开销；Tauri 原生支持 |
| 主 Provider = Codex | 脚本化接口更稳定（--json, --output-schema） |
| 备 Provider = Claude | 兼容模式，利用现有 Claude CLI |
| 剪贴板优先交互 | 避免 macOS 权限问题，不自动模拟 Ctrl+C |
| 窗口初始隐藏，显示时不强制置顶 | 用户反馈长期置顶影响操作；release 配置和显示入口都显式取消 topmost |
| 新增 tauri-plugin-notification | 事件型记忆提醒 MVP 需要本地系统通知；仓库此前无通知能力 |
| 微信集成不进入正式机器人主线 | 自动代发、主动起聊、历史抓取和凭证维护风险高，且与 EchoMate“用户掌控发送”定位冲突 |
| 先做近似机器人 | 与现有热键、剪贴板、截图、本地记忆、提醒架构最兼容 |
| sidecar 仅实验性、只入站 | 把二维码登录和凭证风险隔离在可关闭模块内，避免默认扩大权限面 |
| 复用现有记忆/提醒系统 | 当前已有 `memory_item`、`reminder`、`context_summary`、`reply_feedback`，下一步应补联系人和时间线而不是重建 |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
| Windows E2E runner 卡在窗口定位 PowerShell 子进程，导致桌面看起来未响应 | 1 | 终止 E2E 进程，重启 EchoMate，确认 `responding=True/topmost=False`；runner 增加 `ECHOMATE_E2E_SKIP_MOVE_WINDOW`、窗口句柄等待和 6s 超时 |
| Windows E2E 到点提醒被近期复制冷却规则顺延 | 1 | 新增 `ECHOMATE_E2E_DISABLE_COOLDOWN=1`，只在 E2E 环境跳过冷却；最终完整 E2E 通过 |

## Notes
- 参考设计稿: docs/UI效果图-中文.png (1672x941 PNG)
- 参考研究报告: docs/deep-research-report-EchoMate 本地 AI 回复副驾 MVP 技术研究报告.md
- 产品扩展研究报告: docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md
- 微信机器人集成可行性报告: docs/deep-research-report-EchoMate 与微信机器人集成可行性报告.md
- Goal 模式提示词: docs/goal-mode-prompt-EchoMate 产品扩展实施.md
- 微信近似机器人 Goal 模式提示词: docs/goal-mode-prompt-EchoMate 微信近似机器人实施.md
- 现有项目已初始化 Tauri 2 + 模块结构，依赖已配置
- cargo check 已通过
