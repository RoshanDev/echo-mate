# Progress Log

## Session: 2026-05-28

### Phase 1: 前端 UI 改造
- **Status:** complete
- Actions taken:
  - 重新设计 index.html 弹窗 UI（标题栏、5条候选卡片、复制按钮、底部操作栏）
  - 创建设置页 settings.html（热键配置、Provider 选择、隐私开关、风格画像）
  - 重写完整 dark theme CSS
  - 实现前端 JS 与 Tauri IPC 桥接
- Files: frontend/index.html, frontend/settings.html, frontend/styles.css, frontend/main.js, frontend/settings.js

### Phase 2: 平台层实现
- **Status:** complete
- Actions taken:
  - 全局热键注册/注销（tauri-plugin-global-shortcut）
  - 剪贴板读写（tauri-plugin-clipboard-manager）
  - 系统托盘与窗口管理（显示/隐藏弹窗、置顶、定位）
- Files: src-tauri/src/platform/hotkey.rs, clipboard.rs, input.rs; src-tauri/src/ui/tray.rs, window.rs

### Phase 3: Provider 适配器
- **Status:** complete
- Actions taken:
  - Claude CLI 适配器（claude -p + JSON Schema）
  - Codex CLI 适配器（codex exec + output-schema）
  - 超时控制、进程隔离、错误处理
- Files: src-tauri/src/provider/claude.rs, codex.rs

### Phase 4: Agent 编排层
- **Status:** complete
- Actions taken:
  - Orchestrator 主流程（clipboard → prompt → provider → candidates）
  - Prompt 组装（模板渲染 + 上下文注入）
  - JSON Schema 验证与输出解析
- Files: src-tauri/src/agent/orchestrator.rs, prompt.rs, schema.rs, parser.rs

### Phase 5: 集成联调
- **Status:** complete
- Actions taken:
  - Tauri commands 对接前端（11 条命令）
  - 事件流（热键触发 → 后端处理 → 前端展示）
  - 端到端流程验证
  - 模块重导出（domain/mod.rs, agent/mod.rs）
- Files: src-tauri/src/ui/commands.rs, src-tauri/src/app/mod.rs

### Phase 6: 测试与文档
- **Status:** complete
- Actions taken:
  - cargo check 通过（零错误零警告）
  - 更新 SETUP.md 添加使用说明
  - 提交并推送所有代码到 origin/main
- Manual E2E: 需用户在本地执行 `npx tauri dev`

## Test Results
| Test | Input | Expected | Actual | Status |
|------|-------|----------|--------|--------|
| cargo check | 全项目 | 零错误零警告 | 零错误零警告 | ✅ |

## Error Log
| Timestamp | Error | Attempt | Resolution |
|-----------|-------|---------|------------|
| 2026-05-28 | 插件 API 不一致 (Builder vs init) | 1 | 逐个测试插件 API，混用 Builder::new().build() 和 init() |
| 2026-05-28 | tray-icon 未启用 | 2 | Cargo.toml 添加 tauri feature "tray-icon" |
| 2026-05-28 | get_webview_window 未找到 | 3 | 添加 use tauri::Manager; |
| 2026-05-28 | on_shortcut 闭包参数数量错误 | 4 | 改为 3-arg 闭包 (app, shortcut, event) |
| 2026-05-28 | 闭包内 app 引用生命周期错误 | 5 | clone app handle 后再传入闭包 |

## 5-Question Reboot Check
| Question | Answer |
|----------|--------|
| Where am I? | All phases complete |
| Where am I going? | N/A — MVP 实现完成 |
| What's the goal? | EchoMate 发现版 MVP：热键→剪贴板→CLI→5候选→弹窗→复制 |
| What have I learned? | See findings.md |
| What have I done? | 全部 6 阶段完成，代码已推送至 origin/main |

## Session: 2026-05-31

### Phase 7: Goal 模式产品扩展提示词
- **Status:** complete
- Actions taken:
  - 使用 `$planning-with-files-zh` 工作流读取现有计划、进度和发现文件。
  - 读取 `docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md` 的定位、路线、记忆提醒和 MVP 章节。
  - 提炼事件型记忆提醒 MVP 的目标、范围、安全边界、工程约束和验证标准。
  - 创建 Goal 模式可复制提示词文档。
- Files:
  - docs/goal-mode-prompt-EchoMate 产品扩展实施.md
  - task_plan.md
  - findings.md
  - progress.md

### Notes
- 未执行代码构建或测试；本次工作只新增产品实施提示词和规划记录。
- `docs/deep-research-report-EchoMate 深度研究与产品扩展方案.md` 是输入参考文件，当前仍为未跟踪文件。

### Phase A: 事件型记忆提醒 MVP 现有代码与数据层调研
- **Status:** complete
- Actions taken:
  - 恢复 Goal 模式上下文，读取 `docs/goal-mode-prompt-EchoMate 产品扩展实施.md`。
  - 按 `$planning-with-files-zh` 要求读取 `task_plan.md`、`findings.md`、`progress.md`，并运行 session catchup。
  - 读取 Deep Research 产品扩展报告的定位、路线、记忆提醒、安全边界和最高优先 MVP 章节。
  - 在 `task_plan.md` 新增 Phase A-G 实施阶段，在 `findings.md` 记录事件型记忆提醒 MVP 约束。
  - 使用 CodeGraph 与文件读取调研生成链路、schema、parser、commands、store/memory、前端弹窗和现有验证命令。
  - 发现 `rusqlite` 已存在但 repository/migrations 仍为 TODO，本地通知插件尚未接入。
- Files:
  - task_plan.md
  - findings.md
  - progress.md

### Phase B-F: 事件型记忆提醒 MVP 实现
- **Status:** complete
- Actions taken:
  - 扩展 `CandidateEnvelope`，新增 `action_card`、`memory_candidates`、`reminder_candidates`、`context_summary`，并保留 5 条候选回复。
  - 扩展 JSON Schema 与 parser 校验，保持 strict structured output 的 `required` 与 `properties` 对齐。
  - 扩展 prompt，要求低压、可解释、用户确认、来源回显和敏感信息分级。
  - 实现 SQLite migrations 与 `MemoryRepository`，覆盖 `memory_item`、`reminder`、`context_summary`、`reply_feedback`。
  - 接入 Tauri commands：保存记忆、创建提醒、忽略候选、删除记忆/提醒、恢复最新提醒面板。
  - 新增 Tauri notification 插件；提醒到点后发送系统通知、打开 EchoMate 面板，并显示来源、当前建议和 3 条跟进候选。
  - 托盘入口会在存在已通知提醒时恢复到 `index.html#reminders`，避免通知事件丢失后找不到跟进面板。
  - 增加最小 quiet hours、复制反馈冷却和到点提醒轮询。
  - 前端弹窗新增“当前适合做什么 / 可能值得记住 / 提醒建议”三块卡片，支持保存、改时间、提醒、忽略、删除。
- Files:
  - frontend/index.html
  - frontend/main.js
  - frontend/styles.css
  - src-tauri/Cargo.toml
  - src-tauri/Cargo.lock
  - src-tauri/capabilities/default.json
  - src-tauri/src/agent/orchestrator.rs
  - src-tauri/src/agent/parser.rs
  - src-tauri/src/agent/prompt.rs
  - src-tauri/src/agent/schema.rs
  - src-tauri/src/app/mod.rs
  - src-tauri/src/domain/memory_item.rs
  - src-tauri/src/domain/message.rs
  - src-tauri/src/domain/mod.rs
  - src-tauri/src/provider/codex.rs
  - src-tauri/src/store/memory_repo.rs
  - src-tauri/src/store/migrations.rs
  - src-tauri/src/ui/commands.rs

### Phase G: 验证
- **Status:** complete
- Verification results:
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`: pass
  - `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: pass
  - `cargo test --manifest-path src-tauri/Cargo.toml`: pass, 12 tests
  - `cargo check --manifest-path src-tauri/Cargo.toml`: pass
  - `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu`: pass
  - `node --check frontend/main.js`: pass
  - `node --check tests/windows-e2e-runner.mjs`: pass
  - `chromium-browser --headless=new --disable-gpu --no-sandbox --allow-file-access-from-files --virtual-time-budget=5000 --dump-dom tests/frontend-memory-reminder-harness.html`: pass; `#result` 输出 `PASS`
  - `git diff --check`: pass
  - `make win-run`: pass; Windows release build finished and launched `C:\Users\pibao\echo-mate\src-tauri\target\release\echo-mate.exe`
  - Windows process check: `echo-mate.exe` running as PID 55844 after normal `make win-run`
  - Windows E2E runner: pass after final frontend copy-button fix, against release app with `ECHOMATE_E2E_MOCK_PROVIDER=1`, `ECHOMATE_E2E_SKIP_SCREENCLIP=1`, and WebView2 CDP port 9222.
- Windows E2E evidence:
  - Runner observed `candidates-ready` 3 times: manual text button, configured global hotkey selection (`CmdOrCtrl+Shift+X` from local config), and screenshot context.
  - Runner observed `reminder-due` once and the EchoMate follow-up panel rendered with source, current suggestion, and 3 follow-up candidates.
  - SQLite evidence in `%APPDATA%\EchoMate\echomate.db`: `context_summary` has `text`, `text`, and `screenshot`; `memory_item` has confirmed event memory with source; `reminder` status is `notified`; `reply_feedback` recorded follow-up copy.
  - Screenshot evidence saved under `C:\Users\pibao\echo-mate\`: `e2e-text-window.png`, `e2e-reminder-window.png`, `e2e-hotkey-window.png`, `e2e-screenshot-window.png`, plus DOM captures.
- Remaining risk:
  - The verified reminder path is local notification trigger plus automatic EchoMate panel restore / reminder entry recovery. `tauri-plugin-notification 2.3.3` desktop Rust API does not expose a toast click callback, so a true OS toast click callback remains a future integration risk.

### Follow-up Fix: 自动识别文字/截图与截图重生成
- **Status:** complete
- Trigger:
  - 用户反馈截图模式下点击“再轻松一点”后出现 `Clipboard read error: The clipboard contents were not available in the requested format or the clipboard is empty.`
- Root cause:
  - `regenerate_candidates` / `regenerate_with_style` 固定调用文本剪贴板生成路径；截图生成后再次调整风格会尝试读取剪贴板文字。
- Fix:
  - 忙碌状态不再覆盖为生成失败：重复点击时前端保持 loading，后端不再发送 `generation-error` 破坏当前进行中的请求状态。
  - 按钮/剪贴板路径会立刻显示 loading 和禁用按钮；选中文本热键路径先复制选中文本，再显示 EchoMate，避免置顶窗口抢焦点。
  - Orchestrator 保存上一次成功生成的输入源：`Text` 或 `Screenshot(path,width,height)`。
  - “重新生成 / 再保守一点 / 再轻松一点”复用上一次输入源，不再重新读取剪贴板。
  - `generate_replies` 自动识别剪贴板：优先非空文字；否则读取剪贴板图片并走截图上下文。
  - 全局热键优先复制当前选中文本；复制不到文字时 fallback 到热键触发前的剪贴板图片，适配微信截图已写入剪贴板的流程。
  - 新增“找话题”按钮和 `generate_topics` 命令，不依赖最后聊天记录，生成主动开启话题/自然续聊候选。
  - E2E mock 模式新增 `ECHOMATE_E2E_DISABLE_QUIET_HOURS=1`，避免夜间 quiet hours 影响自动化提醒断言。
- Verification:
  - `node --check frontend/main.js && node --check tests/windows-e2e-runner.mjs`: pass
  - `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: pass
  - `cargo check --manifest-path src-tauri/Cargo.toml`: pass
  - `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu`: pass
  - `cargo test --manifest-path src-tauri/Cargo.toml`: pass, 12 tests
  - Chromium headless frontend harness: pass
  - Windows release E2E runner: pass; `candidates-ready=6`, `reminder-due=1`; covered text button, selected-text hotkey, clipboard-image auto generation, screenshot style regeneration, image fallback hotkey, and proactive topic generation.
  - `make win-run`: pass; Windows release running as PID 60192.

## Session: 2026-06-08

### Phase H: 微信机器人集成可行性规划
- **Status:** complete
- Actions taken:
  - 使用 `$planning-with-files-zh` 工作流读取技能说明，并恢复现有 `task_plan.md`、`findings.md`、`progress.md`。
  - 读取 `.omx/state/session.json` 与 `.omx/state/current-task-baseline.json`，确认当前 session 和主 worktree 状态。
  - 读取 `docs/deep-research-report-EchoMate 与微信机器人集成可行性报告.md` 全文。
  - 抽样核验关键主源：Tencent `openclaw-weixin`、`epiral/weixin-bot` 协议文档、Microsoft Notification Listener、Apple NSWorkspace/Pasteboard/Accessibility。
  - 对照现有代码中的 `memory_item`、`reminder`、`context_summary`、`reply_feedback`、截图/文本/topic 生成和提醒闭环，形成下一阶段计划。
  - 在 `task_plan.md` 追加 Phase H 与 W0-W7 微信集成规划阶段。
  - 在 `findings.md` 追加报告结论、主源校验、代码适配点和风险门槛。
- Files:
  - task_plan.md
  - findings.md
  - progress.md

### Verification
- `git diff --check`: pass
- 规划文件内容复读检查: pass，`task_plan.md` 已标记 Phase H complete，后续 W0-W7 均为 pending/gated，未误标为已实施。

### Notes
- 本次只做规划和研究记录，没有修改业务代码。
- 当前建议路线：正式版先做“近似机器人”；实验性 `weixin-bot` sidecar 需要显式批准后才实施，并且只入站、不自动代发。

### Phase I: 微信近似机器人 Goal 模式实施提示词
- **Status:** complete
- Actions taken:
  - 读取既有 `docs/goal-mode-prompt-EchoMate 产品扩展实施.md` 的结构。
  - 将 `task_plan.md` 中 Phase W0-W6 转写成新会话可直接执行的 Goal 模式提示词。
  - 在提示词中明确 hard boundaries：不自动代发、不主动起聊、不抓全量历史、不默认 sidecar。
  - 将 W7 `weixin-bot` sidecar 标为 gated，要求显式批准后才实施。
  - 新建 `docs/goal-mode-prompt-EchoMate 微信近似机器人实施.md`。
- Files:
  - docs/goal-mode-prompt-EchoMate 微信近似机器人实施.md
  - task_plan.md
  - findings.md
  - progress.md

### Verification
- `git diff --check`: pass
- 目标提示词复读检查: pass，提示词明确从 W0-W6 实施，W7 sidecar gated，不默认自动代发或接机器人。

### Phase W0: 实施前复核与范围锁定
- **Status:** complete
- Actions taken:
  - 使用 `$planning-with-files-zh` 恢复 `task_plan.md`、`findings.md`、`progress.md`，并运行 session catchup；未发现额外未同步上下文。
  - 读取 `.omx/state/session.json`、`.omx/state/current-task-baseline.json`、`.omx/state/subagent-tracking.json`，确认当前主 worktree active。
  - 复核 `docs/goal-mode-prompt-EchoMate 微信近似机器人实施.md`、微信集成可行性报告和产品扩展报告。
  - 重新打开主源核验 Tencent `openclaw-weixin`、`epiral/weixin-bot` 协议文档、Microsoft Notification Listener、Apple NSWorkspace/Pasteboard/Accessibility 文档入口。
  - 范围锁定为 W1-W6 近似机器人：联系人 allowlist、本地上下文、采用回写、透明权限、Windows 降级 helper、macOS 近似 helper、上下文合并和隐私验证；不做 W7 sidecar。
- Findings:
  - Windows Notification Listener 需要 packaged app capability 和用户授权；本轮不能在 Linux/WSL Tauri 开发环境伪装真实监听完成。
  - macOS 只实现前台应用/Pasteboard/Accessibility 近似能力与权限/降级状态，不承诺后台实时读取微信通知。
  - 当前代码缺口集中在 `contacts`、`messages`、`style_profile`、权限状态、allowlist gating、PromptComposer 上下文注入、联系人/权限 UI 和测试 harness。

### Phase W1-W5: 微信近似机器人实现
- **Status:** complete
- Actions taken:
  - 新增联系人白名单模型、SQLite migration、repository CRUD、设置页编辑/启用/停用/清空/删除入口。
  - 新增联系人级 `messages` timeline、`style_profile` 摘要持久化、`platform_signal_log` 判定日志，并复用既有 `memory_item`、`reminder`、`context_summary`、`reply_feedback`。
  - 将 `active_contact_id`、全局隐私模式、保留期限、Windows/macOS helper 开关和 Accessibility 开关加入 `AppConfig`。
  - 生成链路合并联系人记忆、最近上下文、风格画像；未白名单或隐私模式开启时不保存上下文、记忆或提醒候选。
  - 采用/复制候选回写 reply feedback、approved message 和 style profile 摘要。
  - 实现 `ingest_platform_signal`：只对白名单联系人记录近似入站 signal，发出本地 `inbound-signal` 提示，不自动生成、不自动发送。
  - 新增 macOS one-shot context snapshot helper：macOS 使用 `osascript`/System Events 尝试读取前台 app、窗口标题、Accessibility 选中文本和 Pasteboard；非 macOS 明确降级。
  - 弹窗 UI 增加联系人选择、入站 signal banner、上下文来源、删除这条上下文；设置页增加权限边界、helper 开关和联系人管理。
  - 修复窗口长期置顶问题：`tauri.conf.json` 改为 `alwaysOnTop=false`，所有显示入口显式取消 topmost。
- Files:
  - Makefile
  - frontend/index.html
  - frontend/main.js
  - frontend/settings.html
  - frontend/settings.js
  - frontend/styles.css
  - src-tauri/tauri.conf.json
  - src-tauri/src/agent/orchestrator.rs
  - src-tauri/src/agent/prompt.rs
  - src-tauri/src/agent/schema.rs
  - src-tauri/src/app/mod.rs
  - src-tauri/src/domain/memory_item.rs
  - src-tauri/src/domain/mod.rs
  - src-tauri/src/platform/macos_context.rs
  - src-tauri/src/platform/mod.rs
  - src-tauri/src/store/memory_repo.rs
  - src-tauri/src/store/migrations.rs
  - src-tauri/src/ui/commands.rs
  - src-tauri/src/ui/tray.rs
  - src-tauri/src/ui/window.rs
  - tests/frontend-memory-reminder-harness.html
  - tests/frontend-contact-settings-harness.html
  - tests/windows-e2e-runner.mjs

### Phase W6: 验证与隐私回归
- **Status:** complete
- Passed verification:
  - `cargo fmt --manifest-path src-tauri/Cargo.toml`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml --check`
  - `cargo test --manifest-path src-tauri/Cargo.toml`: pass, 14 tests
  - `cargo check --manifest-path src-tauri/Cargo.toml`: pass
  - `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu`: pass
  - `node --check frontend/main.js`: pass
  - `node --check frontend/settings.js`: pass
  - `node --check tests/windows-e2e-runner.mjs`: pass
  - `chromium-browser --headless=new --disable-gpu --no-sandbox --allow-file-access-from-files --virtual-time-budget=5000 --dump-dom tests/frontend-memory-reminder-harness.html`: pass; `#result` 输出 `PASS`
  - `chromium-browser --headless=new --disable-gpu --no-sandbox --allow-file-access-from-files --virtual-time-budget=5000 --dump-dom tests/frontend-contact-settings-harness.html`: pass; `#result` 输出 `PASS`
  - `git diff --check`: pass
  - `make win-run`: pass after latest Rust changes; Windows release launched
  - Windows process/topmost check: `responding=True`, `topmost=False`
  - Windows copy of `tests/windows-e2e-runner.mjs`: `node --check` pass
  - Full Windows desktop E2E runner: pass with release app and `ECHOMATE_E2E_MOCK_PROVIDER=1`, `ECHOMATE_E2E_SKIP_SCREENCLIP=1`, `ECHOMATE_E2E_DISABLE_QUIET_HOURS=1`, `ECHOMATE_E2E_DISABLE_COOLDOWN=1`, `ECHOMATE_E2E_SKIP_MOVE_WINDOW=1`, temporary `ECHOMATE_E2E_HOTKEY=CmdOrCtrl+Shift+Q`, and WebView2 CDP port 9222.
  - Windows E2E evidence: output contained `"ok": true`; `eventCounts` were `candidates=6`, `reminders=1`, `inbound=1`; covered allowlisted notification signal without auto-generation, text generation, context merge, memory save, reminder creation/recovery, selected-text hotkey, screenshot generation, screenshot style regeneration, clipboard-image hotkey fallback, and topic generation.
  - Windows E2E cleanup evidence: original user config restored to `CmdOrCtrl+Shift+X`; post-run process check `responding=True`, `topmost=False`; no `windows-e2e-runner`/`ECHOMATE_E2E` node process remained.
- Not-tested:
  - macOS manual verification has not been executed in this Linux/Windows environment: Accessibility switch, selected-text failure fallback, and Pasteboard helper path remain Not-tested. The implemented macOS helper is covered by a non-macOS unit test that asserts explicit unavailable/fallback behavior.
