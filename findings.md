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

## WeChat Bot Integration Report Findings
- 输入报告：`docs/deep-research-report-EchoMate 与微信机器人集成可行性报告.md`。
- 报告结论可采纳：EchoMate 不应把第三方微信机器人做成正式版默认能力；优先做“近似机器人”，即快捷键、剪贴板、截图、本地记忆、联系人 allowlist、提醒和风格画像。
- 真正 bot 入口只适合作为实验性 sidecar：二维码登录、只订阅入站消息、投递到 localhost、不得自动 `reply()` / `send()`。
- 直接接入 `openclaw-weixin` 不适合首版主线：需要 OpenClaw 宿主、插件兼容矩阵和二维码登录流程，且把 EchoMate 变成外围 agent 会造成架构错位。
- `cli-in-wechat` 适合作为本地桥接和 provider 路由参考，不适合直接作为 EchoMate 产品依赖。
- `epiral/weixin-bot` / iLink 路线相对轻，但凭证保存、长轮询、context token、重登录和平台稳定性都应隔离在实验模块。
- 产品必须坚持：不自动发送、不主动起聊、不做全量历史抓取、不做群监控、不自动保存敏感信息。

## WeChat Integration Source Checks
- Tencent `openclaw-weixin` README 当前说明：安装需要 OpenClaw CLI，快速安装命令是 `npx -y @tencent-weixin/openclaw-weixin-cli install`，登录命令是 `openclaw channels login --channel openclaw-weixin`，并通过二维码确认后本地保存凭证；协议接口包含 `getupdates`、`sendmessage`、`getuploadurl`、`getconfig`、`sendtyping`。Source: https://github.com/Tencent/openclaw-weixin
- `epiral/weixin-bot` 协议文档说明 iLink Bot API 的核心是二维码登录、`getupdates` 长轮询、消息携带 `context_token`，发送回复时需要回传该 token；session 失效后没有观察到刷新 token 独立接口，通常需要重新扫码登录。Source: https://github.com/epiral/weixin-bot/blob/main/docs/protocol-spec.md
- Microsoft Notification Listener 文档说明 Windows 读取通知需要 `User Notification Listener` capability，并且用户必须授权；可读取 toast 通知内容，但清理通知 API 必须谨慎。Source: https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/notification-listener
- Apple 文档支持 macOS 前台应用激活通知、Pasteboard `changeCount`、Accessibility `AXUIElementCopyAttributeValue` 与 `kAXSelectedTextAttribute`；未确认存在与 Windows Notification Listener 同等级的公开跨 app 通知读取能力。Sources: https://developer.apple.com/documentation/appkit/nsworkspace/didactivateapplicationnotification , https://developer.apple.com/documentation/appkit/nspasteboard/changecount , https://developer.apple.com/documentation/applicationservices/axuielement_h , https://developer.apple.com/documentation/applicationservices/kaxselectedtextattribute

## WeChat Integration Codebase Fit
- 现有 EchoMate 已具备“近似机器人”主干：文本剪贴板、截图上下文、找话题、候选回复、next_action、记忆候选、提醒候选、SQLite 本地保存、到点提醒和 Windows E2E。
- 现有 SQLite 表包括 `memory_item`、`reminder`、`context_summary`、`reply_feedback`；下一阶段应补 `contacts`、最近消息 timeline、`style_profile` 持久化，不应重建已有记忆/提醒系统。
- `ContactFact` 和 `StyleProfile` domain struct 已存在，但目前没有对应 migration/repository 闭环。
- 入站提醒应先进入本地 signal 层，再由 allowlist 决定是否显示“要不要生成候选”；不应直接触发 provider 生成或发送。
- Windows helper 的可行性比 macOS 高，但需要新增权限说明和撤权降级逻辑；macOS 应以用户主动切回微信、复制、选中文本或截图为可靠主路径。
- sidecar 若后续实施，应是独立进程、独立开关、独立凭证删除路径；主应用只接收脱敏后的 inbound event。

## WeChat Integration Risks & Gates
- ChatGPT 报告内的引用标记不是可直接复用的本地证据；实施前必须重新打开主源并确认 API、包名、兼容矩阵和许可状态。
- 任何机器人形态都会扩大隐私和误发风险；正式发布前必须有“0 自动发送”自动化断言。
- Windows Notification Listener 读取的是用户通知流，权限敏感；不能默认开启，也不能在 EchoMate 里提供“清空所有通知”类操作。
- macOS Accessibility 容易受到目标应用控件支持度影响；失败必须回退到现有复制/截图路径。
- 不新增依赖是当前项目默认约束；`weixin-bot` Node sidecar 需要用户显式批准后再引入。

## WeChat Goal Prompt Findings
- 新会话实施提示词已写入 `docs/goal-mode-prompt-EchoMate 微信近似机器人实施.md`。
- 该提示词要求新会话从 Phase W0 开始，完成 W0-W6，并使用 `$planning-with-files-zh` 持续更新 `task_plan.md`、`findings.md`、`progress.md`。
- 提示词明确 W7 `weixin-bot` sidecar 为 gated，不默认实施；不自动代发、不主动起聊、不抓全量历史仍是硬边界。
- 提示词把 Windows Notification Listener 标为必须先复核的可选能力；如果当前 Tauri/打包形态不可用，新会话必须实现安全降级并记录风险，不能伪装完成。

## WeChat Approx Bot W0 Source Recheck
- 2026-06-08 复核 Tencent `openclaw-weixin` 主源：当前 README 仍要求先安装 OpenClaw CLI，插件版本与 OpenClaw 版本有兼容矩阵；登录仍是二维码授权并本地保存凭证，协议接口仍是 iLink HTTP JSON API，包括 `getupdates` 长轮询、`sendmessage`、`getuploadurl`、`getconfig`、`sendtyping`，回复仍需要 `context_token`。Source: https://github.com/Tencent/openclaw-weixin
- 2026-06-08 复核 `epiral/weixin-bot` 协议主源：仍是 iLink Bot API 路线，核心能力为扫码、长轮询入站消息、基于上下文 token 回传回复；适合后续 gated sidecar 实验，不适合正式主线默认接入。Source: https://github.com/epiral/weixin-bot/blob/main/docs/protocol-spec.md
- 2026-06-08 复核 Microsoft Notification Listener：Windows 读取用户通知仍需要应用声明 User Notification Listener capability，并由用户显式授权；这与当前 Tauri Linux/WSL 构建环境不等价，不能把真实 Windows 通知监听伪装为已完成。正式实现本轮采用可选权限状态/降级模型和本地 signal 入口，真实 packaged capability 留作 Windows 后续验证项。Source: https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/notification-listener
- 2026-06-08 复核 Apple 文档：`NSWorkspace.didActivateApplicationNotification`、`NSPasteboard.changeCount`、`AXUIElement` 与 `kAXSelectedTextAttribute` 文档入口仍存在；Apple 文档页需要 JavaScript 才能展开全文。本轮仅实现 macOS 近似上下文的权限/降级模型，不承诺后台实时读取微信通知。Sources: https://developer.apple.com/documentation/appkit/nsworkspace/didactivateapplicationnotification , https://developer.apple.com/documentation/appkit/nspasteboard/changecount , https://developer.apple.com/documentation/applicationservices/axuielement_h , https://developer.apple.com/documentation/applicationservices/kaxselectedtextattribute
- 本轮范围锁定：只做近似机器人；不实现自动代发、主动起聊、全量历史抓取、群监控、默认 OpenClaw/weixin-bot sidecar 或新增 Node 依赖。
- 代码适配结论：现有 `memory_item`、`reminder`、`context_summary`、`reply_feedback` 可复用；当前缺口是 `contacts`、`messages`、`style_profile`、联系人级删除/保留、权限降级状态、allowlist gating、PromptComposer 上下文注入、前端联系人/权限/来源/删除 UI。

## WeChat Approx Bot Implementation Findings
- W1/W2 实现采用现有 SQLite/Rust 架构，不新增依赖：新增 `contacts`、`messages`、`style_profile`，并给 `memory_item`、`context_summary`、`reply_feedback` 增加 contact/candidate 关联字段。
- 生成链路通过 `active_contact_id`、联系人 allowlist、`global_privacy_mode` 和 `context_retention_days` 控制上下文保存；未选择白名单联系人或全局隐私模式开启时，生成仍可给 5 条候选，但不会保存上下文、记忆或提醒候选。
- 采用/复制候选会写入 `reply_feedback` 并更新 `style_profile` 摘要统计，同时把用户确认采用的回复作为 approved message；style profile 只存摘要和计数，不保存无限原文样本。
- Windows Notification Listener 本轮没有伪装成真实 packaged capability：实现了权限状态、显式开关、`ingest_platform_signal` 本地信号入口、白名单 gate、`inbound-signal` UI 提示和 `platform_signal_log` 判定日志；未白名单、隐私模式或 helper 关闭时不保存入站内容。
- macOS helper 本轮是无新增依赖的一次性上下文快照：macOS 下通过 `osascript`/System Events 尝试读取前台 app、窗口标题、Accessibility 选中文本和 Pasteboard；非 macOS 返回明确不可用，失败时保留复制/热键/截图路径。
- 前端设置页增加联系人白名单、全局隐私、保留期限、Windows/macOS helper 开关、Accessibility 开关、权限边界说明和联系人级清空/删除入口；弹窗增加当前联系人、上下文来源、删除这条上下文和入站 signal banner。
- 用户反馈窗口长期置顶影响操作后，release 配置从 `alwaysOnTop: true` 改为 `false`，所有显示入口都显式 `set_always_on_top(false)`；Windows release 重启后 WinAPI 检查 `topmost=False`。
- Windows E2E runner 原先在窗口定位 PowerShell 子进程中卡住，已增加窗口句柄等待、6s 超时、`ECHOMATE_E2E_SKIP_MOVE_WINDOW=1` 跳过开关和 Windows 副本同步；最终完整桌面 E2E 已通过。
- Windows E2E 为了稳定验证提醒恢复，新增 `ECHOMATE_E2E_DISABLE_COOLDOWN=1` 只在测试环境跳过“近期复制则顺延提醒”的冷却规则；生产默认仍保留冷却。
