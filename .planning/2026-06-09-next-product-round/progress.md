# EchoMate 下一轮开发进度

## 2026-06-09

本次目标：参考 deep-research 报告，规划下一轮开发计划。

已完成：

- [x] 按 `planning-with-files` 技能检查现有计划文件。
- [x] 确认根目录 `task_plan.md`、`findings.md`、`progress.md` 是历史 MVP/微信集成计划，保留不覆盖。
- [x] 阅读 `AGENTS.md` 中关于真实用户上下文、e2e/mock 隔离、截图夹具的硬规则。
- [x] 阅读 `docs/deep-research-report-EchoMate 深度研究与产品扩展方案-2026-06-09.md` 的产品判断、功能优先级、路线图、记忆提醒、截图理解、微信集成和隐私发布章节。
- [x] 建立新的活动计划目录 `.planning/2026-06-09-next-product-round/`。
- [x] 写入下一轮 `task_plan.md`、`findings.md`、`progress.md`。
- [x] 追加“联系人资料补充入口 / 手动事实 / EchoMate 自主引用决策”到计划。

当前状态：

- Phase 0 规划完成。
- 下一步应从 Phase 1 “上下文完整性与来源追踪”开始实施。

下一步建议：

- [ ] 设计 source/provenance 相关 schema 和迁移。
- [ ] 设计 `contact_facts` 手动资料 schema、归类 prompt 和引用策略。
- [ ] 定义 provider 输出 JSON schema 与错误模型。
- [ ] 实现来源卡 UI 的最小版本。
- [ ] 补全联系人清除上下文的回归测试。
- [ ] 补全 e2e/mock 污染检测测试。

## 2026-06-09 Phase 1 实施启动

已完成：

- [x] 读取 `goal_prompt.md` 并创建 active goal。
- [x] 按计划恢复上下文：读取 `AGENTS.md`、`.active_plan`、`task_plan.md`、`findings.md`、`progress.md` 和 deep-research 报告开头/关键索引。
- [x] 运行 `session-catchup.py`，未发现需要同步的上次会话摘要输出。
- [x] 检查 `git status --short`：当前仅 `.planning/` 与新 deep-research 文档处于未跟踪状态，未发现已修改代码文件。

当前判断：

- Phase 1 的第一批落点应先围绕现有 DB/provider/UI 数据流做最小贯通：来源记录、手动资料 facts、provider JSON 诊断、清除语义与测试隔离。

## 2026-06-09 Phase 1 第一批实现

已完成：

- [x] 新增并迁移 `source_contexts`、`suggestion_runs`、`contact_facts`，并给 `messages`、`context_summary`、`memory_item`、`reminder`、`reply_feedback` 补来源/时间/运行关联字段。
- [x] 新增 `ContactFactCandidate`、`ContactFactClassification`、`ContactFactRecord`、`SourceCard`、`SourceContextRecord`、`SuggestionRunRecord` 等 domain 类型。
- [x] 生成链路写入 source context 和 suggestion run；主弹窗 payload 返回 `source_cards`，UI 展示当前输入、用户手动补充、已批准记忆和 provider 调用来源。
- [x] prompt 合同新增“只能使用列出的上下文来源”“手动补充资料不是聊天记录”“敏感/无关资料默认不用”等规则。
- [x] 设置页新增联系人补充资料入口：自然语言输入、provider 归类、预览、用户确认保存、事实列表、删除。
- [x] 新增 `contact_fact_schema()` 和手动资料归类 prompt；示例和测试 fixture 改成匿名假数据（联系人A/A 市/B 市），避免真实联系人数据进入代码。
- [x] `找话题` 新增可选 hint 输入框；用户不填时由 EchoMate 自行找话题，填写时作为本次方向提示进入 prompt/source card，不保存为聊天记录。
- [x] `clear_contact_context` / `delete_contact` 覆盖 `messages`、`reply_feedback`、`platform_signal_log`、`context_summary`、`memory_item`、`reminder`、`source_contexts`、`suggestion_runs`、`contact_facts`。
- [x] Codex/Claude provider 解析补充空输出、非 JSON、schema drift、stderr/stdout 预览的诊断；手动 fact 归类也走结构化解析。
- [x] e2e/mock 隔离增强：`ECHOMATE_E2E_MOCK_PROVIDER=1` 时 DB/config 强制走临时 profile；macOS runner 禁止测试运行中的真实 app；Windows runner 要求临时 `APPDATA`/`ECHOMATE_E2E_PROFILE_DIR`。

用户反馈处理：

- [x] 针对截图中“找话题候选过泛、像尬聊”的反馈，新增找话题 hint 输入，避免完全由模型自由发挥。
- [x] 针对“别在代码里泄露用户数据”的反馈，清理 `src-tauri`、`frontend`、`tests` 中真实式联系人别名/城市示例；保留的测试联系人均为匿名假数据。

验证：

- [x] `cargo fmt --manifest-path src-tauri/Cargo.toml`
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`：21 passed。
- [x] `node --check frontend/main.js && node --check frontend/settings.js && node --check tests/windows-e2e-runner.mjs && node --check tests/macos-smoke-runner.mjs`
- [x] `rg` 检查 `src-tauri`、`frontend`、`tests` 中的真实式联系人/城市示例：无匹配。

仍未完成：

- [ ] 截图内可见时间仍未做 OCR/结构化抽取，只记录 `visible_message_time`/`inferred_chat_time` 字段并在无法确定时标记 `unknown`。

## 2026-06-10 全 Phase 实施启动

本次目标：用户要求“完成所有 Phase，使用 goal 模式”。

已完成：

- [x] 尝试创建新的 active goal；工具返回同一线程已有已完成 goal，不能创建第二个 goal。
- [x] 读取当前 goal 状态：上一条 Phase 1 goal 已 `complete`。
- [x] 读取 `planning-with-files` 技能、`task_plan.md`、`progress.md`、`findings.md` 和当前 git 状态。
- [x] 运行 `session-catchup.py`，未输出需要同步的遗漏上下文。

当前判断：

- 本次按同一计划目录继续推进 Phase 2-7。
- 必须继续遵守真实 profile 隔离规则：不运行会碰真实 EchoMate profile 的 e2e/mock 流程；如需持久化测试必须使用临时目录。
- Phase 1 只剩截图内可见时间/OCR 抽取这一项，和 Phase 2 截图理解 v2 合并处理。

## 2026-06-10 Phase 2-7 第一批接线

已完成：

- [x] 扩展 provider envelope/domain：候选增加 `intent_group`、`source_refs`；输出增加 `situation`、`source_summary`、`screenshot_analysis`。
- [x] 扩展 JSON schema：要求统一输出 situation/action/source summary/memory candidates/reminder candidates/context summary/screenshot analysis。
- [x] 新增 macOS Apple Vision best-effort OCR helper：运行时用 `/usr/bin/swift` 调 Vision，本地 OCR 失败时返回 warning 并走 provider 视觉补偿。
- [x] 截图生成链路加入本地 OCR/左右气泡/可见时间启发式解析，并把解析结果写入 prompt、source context metadata、`screenshot_analyses`。
- [x] 迁移扩展：`screenshot_analyses`；`memory_candidates` 增加 summary/source_quote/reason/ttl_days；`reminder` 增加 contact/kind/due/source/cooldown/snooze 字段。
- [x] Repository 增加记忆候选收件箱、确认/忽略、提醒中心、完成/延后、关系卡、数据审计、数据导出快照、全量清除 API。
- [x] Tauri commands 暴露关系卡、候选记忆收件箱、提醒中心、数据审计、隐私向导状态等入口。
- [x] 主弹窗增加截图理解展示，候选卡显示 intent/source refs。
- [x] 设置页增加隐私向导、关系卡、记忆候选收件箱、提醒中心、数据审计/导出/清空入口。
- [x] 移除前端 Tauri shell execute capability；Provider CLI 仍由 Rust 后端受控调用。

待验证/修正：

- [x] 运行 `cargo fmt`/`cargo test` 后修复新增字段、SQL 和 schema 测试问题。
- [x] 运行 JS syntax check。

## 2026-06-10 Phase 2-7 验证收尾

已完成：

- [x] 补提醒静默表 `reminder_mutes`，提醒中心支持静默联系人/静默提醒类型。
- [x] 提醒 loop 加入低打扰频率控制：同联系人 24 小时最多通知 1 次、7 天最多 2 次，命中则自动延后。
- [x] 增加清理日志命令和设置页入口。
- [x] 移除前端 `shell:allow-execute` capability 和未使用的 `tauri-plugin-shell` 依赖。
- [x] 移除 Claude stdout debug 文件落盘，Claude stderr 诊断改为截断预览；热键触发失败日志不再写入完整 provider 错误正文。
- [x] 新增 GitHub Actions：macOS/Windows 测试和 Tauri bundle，macOS signing/notarization 作为 secrets gate。
- [x] 扩展仓储回归测试：记忆候选收件箱确认、提醒中心、审计/导出/清空、关系卡、截图解析持久化。

验证：

- [x] `cargo fmt --manifest-path src-tauri/Cargo.toml`
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`：22 passed。
- [x] `node --check frontend/main.js && node --check frontend/settings.js && node --check tests/windows-e2e-runner.mjs && node --check tests/macos-smoke-runner.mjs`
- [x] `rg -n "齐齐|天津|上海工作|93 年|小周|在B 市|1990 后" src-tauri frontend tests`：无匹配。
- [x] `rg -n "shell:allow|shell:default|tauri_plugin_shell|tauri-plugin-shell|last-claude-output|Orchestrator trigger error" src-tauri frontend tests`：无匹配。

严格遗留项：

- [ ] 多截图“按选择顺序拼接”的完整交互尚未做成；当前只完成截图解析 schema、turns 承载和单截图本地 OCR/视觉补偿。
- [ ] 记忆候选收件箱已支持记住/忽略，但候选编辑和手动设置过期时间还不是完整 UI。
- [ ] 已保存记忆的“最后使用时间/被哪些建议引用过”仍需更细的 usage tracking UI；当前可通过来源卡、关系卡和数据审计追踪主要来源。

## 2026-06-09 Phase 1 补齐来源表和污染扫描

已完成：

- [x] 新增并迁移 `message_events` 专表；`append_message_with_source_context` 写入消息时同步记录 provider、input kind、fact source、source context、捕获时间、可见时间、推断聊天时间和来源置信度。
- [x] 新增并迁移 `memory_candidates` 专表；每次 `suggestion_run` 后持久化 provider 输出的候选记忆，但状态保持为 `candidate`，不会直接进入长期记忆。
- [x] 补齐 `suggestion_runs` 的 `fact_source`、`captured_at`、`visible_message_time`、`inferred_chat_time`、`source_confidence` 字段，使 run 记录不只依赖间接 JSON。
- [x] 补齐 `contact_facts` 的 provenance 字段和旧库迁移补列；手动资料保存为 `input_kind=manual`，不会进入 `messages`。
- [x] 新增 `scan_for_test_artifacts()` 污染扫描器，检查 `contacts`、`messages`、`message_events`、`memory_item`、`memory_candidates`、`context_summary`、`platform_signal_log`、`source_contexts`、`suggestion_runs`、`reply_feedback`、`contact_facts` 中的 `e2e/mock/test/测试联系人` 标记，并只返回命中的标记而不回传完整正文。
- [x] 清理上下文和删除联系人现在覆盖 `message_events`、`memory_candidates`、`suggestion_runs`、`source_contexts`、`contact_facts` 等来源/候选数据。
- [x] 回归测试补充：清空联系人后 `message_events`、`memory_candidates`、`suggestion_runs`、`contact_facts` 均清零；污染扫描器能发现 mock/test 标记且不泄露完整 fixture 文本；mock provider 默认 DB 走临时 profile。

验证：

- [x] `cargo fmt --manifest-path src-tauri/Cargo.toml`
- [x] `cargo test --manifest-path src-tauri/Cargo.toml`：22 passed。
- [x] `node --check frontend/main.js && node --check frontend/settings.js && node --check tests/windows-e2e-runner.mjs && node --check tests/macos-smoke-runner.mjs`
- [x] `rg -n "齐齐|天津|上海工作|93 年|小周|在B 市|1990 后" src-tauri frontend tests`：无匹配。

仍未完成：

- [ ] 截图内可见时间仍未做 OCR/结构化抽取，只记录 `visible_message_time`/`inferred_chat_time` 字段并在无法确定时标记 `unknown`。
