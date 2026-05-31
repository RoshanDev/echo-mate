# Findings & Decisions

## Requirements
- 全局热键触发（CmdOrCtrl+Shift+Space）
- 读取系统剪贴板文本
- 调用 Codex CLI 生成 5 条候选回复（主）
- 调用 Claude CLI 作为备用（备）
- 弹窗展示 5 条候选，每条带风格标签和复制按钮
- 底部操作：再保守一点、再有趣一点、重新生成、打开历史
- 系统托盘常驻，右键菜单
- 设置页：热键、Provider、隐私开关、风格画像配置
- 本地存储候选集和审计日志（非原文）
- 严格隐私模式：不记录原文，只记 hash/统计

## Research Findings
- Tauri 2 tray-icon 需在 Cargo.toml 显式启用 feature
- tauri-plugin-global-shortcut on_handler 在主线程运行
- tauri-plugin-clipboard-manager 有 read_text/write_text API
- Codex CLI 需要 `--skip-git-repo-check`（非 git 仓库场景）
- Codex `--output-schema` 支持 JSON Schema 校验输出
- Claude CLI `--json-schema` + `--tools ""` 可做纯生成器
- macOS 12.7.6 支持 Tauri 2 (需要 10.15+)
- 前端需用 `@tauri-apps/api` 访问 Tauri API

## Technical Decisions
| Decision | Rationale |
|----------|-----------|
| 前端用 vanilla HTML/CSS/JS | 弹窗简单，无框架开销；原生 Tauri 支持 |
| Rust 后端主导逻辑 | 安全、与 Tauri 无缝集成、tokio 异步子进程 |
| tokio::process 托管 CLI | kill_on_drop + timeout 语义清晰 |
| 窗口初始隐藏 | 热键触发时才显示，托盘常驻 |
| env_clear() + 白名单 | 最小化环境泄漏到子进程 |
| 日志只记元数据 | 保护用户聊天隐私 |

## Issues Encountered
| Issue | Resolution |
|-------|------------|
| Tauri 插件 API 不一致（Builder vs init）| 每个插件查阅源码后使用正确 API |
| tray-icon feature 未启用 | 在 Cargo.toml tauri 依赖中添加 |

## Resources
- Tauri 2 文档: https://v2.tauri.app/
- Codex CLI 文档: codex exec --help
- Claude CLI 文档: claude -p --help
- 项目结构: src-tauri/src/ (9 模块)
- 前端: frontend/ (index.html, main.js, styles.css)
- 设计报告: docs/deep-research-report-EchoMate 本地 AI 回复副驾 MVP 技术研究报告.md

## Product Expansion Findings
- Deep Research 报告建议 EchoMate 从“一次性回复生成器”升级为“个人关系 CRM + AI 回复副驾”。
- 最高优先级 MVP 是“事件型记忆提醒”：从明确聊天事件中提取记忆/提醒候选，用户确认后本地保存，到点提醒并给跟进候选。
- 产品边界必须保持：绝不自动发送、不做 PUA/操控、不偷偷扫描聊天数据库、不自动推断高敏信息。
- 记忆和提醒都应有来源回显、用户确认、删除/忽略路径，避免 creepy 感。
- 下一阶段实现提示词已写入 docs/goal-mode-prompt-EchoMate 产品扩展实施.md，要求结合 `$planning-with-files-zh` 持久化计划和进度。

## Event Memory Reminder MVP Constraints
- EchoMate 的下一阶段定位是“个人关系 CRM + AI 回复副驾”：当下给自然候选回复，长期帮助用户记住明确事件、偏好、禁忌和跟进机会。
- 第一轮只做闭环 MVP：文本/截图触发 → 生成 5 条候选回复 → 输出下一行动、记忆候选、提醒候选 → 用户确认 → 本地保存 → 到点通知 → 打开跟进建议。
- 下一行动类型限定为 `continue_chat`、`wrap_up`、`light_follow_up`、`do_not_push`、`safe_repair`、`soft_invite_candidate`，必须给原因和置信度，避免强断言。
- 记忆类型限定为 `event`、`preference`、`boundary`、`stress_point`、`relationship_milestone`；每条必须带 `source_kind`、`source_ref` 或 `source_excerpt`。
- 敏感分级为普通 / 中 / 高 / 禁止；不要自动推断生理期、病史、住址、定位规律、家庭矛盾等高敏信息。
- 提醒默认最多一到两次，不自动发送消息，不做高频复活僵尸会话；到点只提醒用户，并给低压 follow-up 候选。
- 数据最小化：不自动导入历史聊天，不偷偷扫描微信数据库，不常驻监听全部剪贴板。
- UI 必须紧凑嵌入现有弹窗，不做营销页；保存记忆、创建提醒必须是显式点击，所有候选可忽略。

## Event Memory Reminder Code Findings
- 生成主链路是 `Orchestrator` 读取文本/截图，`PromptComposer` 拼 prompt，`schema::candidate_schema()` 写入 schema，Provider 返回 `CandidateEnvelope`，再通过 `candidates-ready` 事件给前端。
- 现有 `CandidateEnvelope` 只有 `candidates`，前端只渲染候选回复；新增产品卡片应扩展 envelope 与事件 payload，保留 5 条候选回复不变。
- `src-tauri/src/store/memory_repo.rs`、`chat_repo.rs`、`migrations.rs` 仍是 TODO；但 `rusqlite` 已存在，可直接实现最小 SQLite 存储，不需要引入数据库依赖。
- 当前 Tauri 插件包含 global-shortcut、clipboard、shell；本地通知还未接入。实现到点系统通知需要新增 Tauri notification 插件，并在计划/进度中记录原因。
- 前端是 vanilla HTML/CSS/JS，主弹窗高度 480、列表滚动；新增卡片应放在候选列表上方并保持紧凑，避免挤压候选列表。
- 现有测试集中在 schema strict required、Provider 假 CLI 输出解析、进程超时、WSL 路径；新增实现需要补 schema、parser、repository 的单元测试。

## Event Memory Reminder Implementation Findings
- 新增 `tauri-plugin-notification` 是必要依赖：仓库此前只有托盘、热键、剪贴板和 shell，无法发送系统通知。
- 桌面端 Rust 通知 API 能发系统通知，但当前实现采用“到点通知 + 自动打开/聚焦 EchoMate 小面板 + reminder-due 事件 + 托盘提醒入口”的方式恢复跟进面板；真实“点击通知回调”仍需后续确认 Tauri 桌面通知 action/listener 能否稳定覆盖 Windows。
- 提醒调度是进程内轮询，应用重启后会重新扫描 SQLite 中 `scheduled` 状态且已到点的提醒；不是 OS 级离线定时任务。
- “近期已联系则抑制”当前用复制候选反馈作为最小代理信号：20 分钟内复制过候选会把到期提醒顺延 30 分钟。
- 前端闭环用 `tests/frontend-memory-reminder-harness.html` 通过 Chromium headless 验证；没有新增 npm/dev 依赖，避免引入 Playwright test 包。
- Windows UI Automation 当前只能稳定发现 EchoMate 窗口和 WebView2 容器，不能稳定枚举 HTML 内部控件；最终采用 WebView2 CDP 调试端口 + 真实 release app + PowerShell 剪贴板/热键/截图完成 E2E，不引入新依赖。
- Windows E2E runner 会读取本机实际热键配置；本机 `%APPDATA%\EchoMate\config.json` 当前是 `CmdOrCtrl+Shift+X`，不是默认 `CmdOrCtrl+Shift+Space`。
- 本地 `tauri-plugin-notification 2.3.3` 桌面 Rust API 只暴露 builder/show 权限接口，没有可用的 toast click callback；因此点击恢复采用“通知触发时自动打开/聚焦面板 + 托盘提醒入口兜底”。
- E2E 证据确认了文本触发、配置热键选中文本触发、截图上下文触发、保存记忆、创建提醒、`reminder-due` 面板恢复和跟进候选复制；仍未单独证明 Windows 系统 toast 点击回调本身稳定可用。
- 用户反馈截图模式后点击“再轻松一点”会报剪贴板文字读取错误；根因是风格重生成总是走文本剪贴板路径。修复为保存上一次成功输入源，重生成复用文字或截图原始上下文。
- 微信截图已进剪贴板时，自动生成路径应先用文字；文字不可用时读取剪贴板图片。热键路径先尝试复制当前选中文本，复制不到时 fallback 到热键前的剪贴板图片。
- 选中文本热键路径不能在复制选区前弹出 EchoMate；置顶窗口会抢焦点，导致模拟 Ctrl+C 复制失败。按钮触发可立即显示 loading，选区热键必须复制完成后再显示面板。
- 新增“找话题”模式：不依赖最后聊天记录，生成低压主动开场/续聊候选；重生成和风格调整会继续复用 topic 输入源。
