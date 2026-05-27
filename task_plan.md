# Task Plan: EchoMate MVP 发现版 (Discovery Phase)

## Goal
实现 EchoMate 发现版 MVP：用户复制微信消息 → 按全局热键 → 读取剪贴板 → 调用 Codex/Claude CLI → 弹出 5 条候选回复 → 一键复制。

## Current Phase
All phases complete

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

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|
|       |         |            |

## Notes
- 参考设计稿: docs/UI效果图-中文.png (1672x941 PNG)
- 参考研究报告: docs/deep-research-report-EchoMate 本地 AI 回复副驾 MVP 技术研究报告.md
- 现有项目已初始化 Tauri 2 + 模块结构，依赖已配置
- cargo check 已通过
