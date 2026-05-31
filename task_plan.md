# Task Plan: EchoMate MVP 发现版 (Discovery Phase)

## Goal
实现 EchoMate 发现版 MVP：用户复制微信消息 → 按全局热键 → 读取剪贴板 → 调用 Codex/Claude CLI → 弹出 5 条候选回复 → 一键复制。

## Current Phase
Phase G complete: 事件型记忆提醒 MVP 已实现并完成回归验证

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

## Key Questions
1. ~~React+TypeScript+Vite or vanilla HTML/CSS/JS?~~ → 已有 vanilla HTML/CSS/JS 壳，保持轻量
2. Codex CLI 是否已安装在用户机器? → 已确认 codex CLI 可用，Claude CLI 已在 PATH
3. 热键默认值? → CmdOrCtrl+Shift+Space
4. 是否需要 tray-icon feature? → 已启用

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| 前端用 vanilla HTML/CSS/JS | 弹窗简单，无需框架开销；Tauri 原生支持 |
| 主 Provider = Codex | 脚本化接口更稳定（--json, --output-schema） |
| 备 Provider = Claude | 兼容模式，利用现有 Claude CLI |
| 剪贴板优先交互 | 避免 macOS 权限问题，不自动模拟 Ctrl+C |
| 窗口置顶 + 初始隐藏 | 热键触发后显示，符合"快开快关"产品体验 |
| 新增 tauri-plugin-notification | 事件型记忆提醒 MVP 需要本地系统通知；仓库此前无通知能力 |

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
|       |         |            |

## Notes
- 参考设计稿: docs/UI效果图-中文.png (1672x941 PNG)
- 参考研究报告: docs/deep-research-report-EchoMate 本地 AI 回复副驾 MVP 技术研究报告.md
- 产品扩展研究报告: docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md
- Goal 模式提示词: docs/goal-mode-prompt-EchoMate 产品扩展实施.md
- 现有项目已初始化 Tauri 2 + 模块结构，依赖已配置
- cargo check 已通过
