# EchoMate

EchoMate 是一个面向桌面微信等聊天工作流的本地 AI 回复副驾。它不是 HTTP 服务，而是一个 Tauri 2 桌面应用：从剪贴板、当前选中文本、单张/多张截图或本地近似平台信号中读取上下文，调用本机 Claude CLI / Codex CLI 生成候选回复、找话题建议、记忆候选和提醒候选。

EchoMate 的产品边界是“辅助决策与复制候选”：它会展示候选、洞察和本地记忆建议，但不会自动发送消息，也不会绕过聊天软件权限直接读取或操控真实账号。

## 当前功能

### 回复与上下文生成

- **剪贴板生成**：读取剪贴板文字或图片，生成多条候选回复。
- **选中文本热键生成**：通过全局快捷键复制当前选中文本并生成回复，默认热键为 `CmdOrCtrl+Shift+Space`。
- **单截图生成**：通过“框选截图”把聊天截图作为上下文生成候选回复。
- **多截图按序生成**：可连续添加多张聊天截图，再按添加顺序合并上下文生成回复。
- **主动找话题**：针对当前联系人和本地上下文生成低压力、非打扰式的话题开场建议，可输入可选参考提示。
- **候选操作**：候选可一键复制；复制行为会用于本地风格画像统计。
- **重新生成**：支持按保守/有趣等风格调整后重新生成。

### 洞察、记忆与提醒

- **行动建议卡**：展示“继续聊、收尾、轻跟进、不要强推、安全修复”等当前适合动作。
- **上下文来源卡**：说明本次结果来自剪贴板、截图、通知/平台信号、手动上下文或找话题模式。
- **截图理解卡**：截图路径会展示模型对截图聊天内容的理解摘要。
- **记忆候选**：模型只提出“可能值得记住”的候选，用户确认后才保存为本地记忆。
- **提醒候选与提醒中心**：模型可提出提醒建议；用户确认后进入提醒中心，支持完成、稍后提醒、静音和删除。
- **关系卡**：按联系人聚合已保存资料、上下文、记忆和互动线索。

### 联系人与隐私控制

- **联系人白名单**：当前联系人必须在白名单中启用后才会参与上下文和记忆工作流。
- **联系人补充资料**：可手动输入联系人资料，由 provider 归类为事实候选，确认后保存。
- **严格隐私模式**：默认开启，不记录原始消息全文。
- **全局隐私模式**：只生成候选，不保存上下文、记忆或提醒。
- **上下文保留天数**：默认 30 天，可在设置中调整。
- **SQLCipher 开关**：支持使用 SQLCipher 加密本地 SQLite 数据库。
- **调试正文日志开关**：默认关闭；只有明确确认隐私风险后才应开启。
- **数据审计**：可查看本地数据概况、导出快照、清理日志或清空全部本地数据。

### Provider 与平台能力

- **Claude CLI provider**：使用本机 `claude` CLI 的非交互模式，要求用户已在本机完成登录/授权。
- **Codex CLI provider**：使用本机 `codex exec`；截图输入当前优先依赖 Codex 图片能力。
- **主/备用 provider**：设置页可选择主 provider、备用 provider 或无备用。
- **macOS 近似上下文 helper**：可选读取前台/Pasteboard/Accessibility 近似上下文，失败会降级。
- **Windows 通知 helper**：预留可选通知上下文入口，需系统授权，失败会降级。
- **系统托盘与通知**：应用注册托盘入口，并使用桌面通知提示提醒。

## 环境要求

- Rust 工具链，项目声明 `rust-version = 1.77.2`。
- Node.js 与 npm。
- Tauri 2 CLI；可通过 `npm exec --package @tauri-apps/cli@^2.11.0 -- tauri ...` 或 `npx tauri ...` 调用。
- 桌面运行环境：macOS / Windows / Linux。热键、截图和平台 helper 依赖对应系统权限。
- 可选但推荐：`claude` CLI 和/或 `codex` CLI，并已在本机完成登录/授权。

平台备注：

- macOS 上热键复制选中文本需要给终端或打包后的 EchoMate 授予 Accessibility 权限。
- Windows 原生构建可使用仓库里的 `build-windows.bat` 或 Makefile 的 Windows 同步/构建入口。
- Linux/WSL2 下可以编译和做部分开发验证，但全局热键、截图、前台窗口上下文等能力可能受桌面会话限制。

## 安装依赖

```bash
npm install
```

Rust 依赖由 Cargo 在首次 `cargo check` / `cargo build` / `tauri dev` 时拉取。首次编译包含 SQLite/SQLCipher 相关 native 依赖，耗时可能较长。

## 启动与构建

### 日常开发

推荐使用 Tauri dev 模式启动完整桌面应用：

```bash
npx tauri dev
```

或使用 Makefile：

```bash
make dev
```

注意：直接运行 `src-tauri/target/debug/echo-mate` 可能缺少 Tauri dev 资源上下文；开发验证优先使用 `npx tauri dev` / `make dev`。

### 常用检查与构建

```bash
make check
make build
npm run build:tauri
```

等价的 Cargo 命令：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

### Windows 辅助入口

```bash
make sync-win
make win-build
make win-run
```

也可以在 Windows 环境执行：

```bat
build-windows.bat
```

## 使用方式

1. 启动 EchoMate。
2. 进入设置页，确认主/备用 provider、候选数量、超时、隐私开关和热键。
3. 在“联系人白名单”中创建一个联系人，并保持“启用”。
4. 回到主窗口，在顶部联系人下拉框选择当前联系人。
5. 复制一段聊天文本后点击 `⚡ 生成回复`，或选中聊天文本后按全局热键。
6. 如需截图上下文，点击 `▣ 框选截图`；如需多轮截图，使用“＋ 添加截图”添加多张，再点击“按顺序生成”。
7. 如需主动开启话题，可填写“找话题参考（可空）”，点击 `💬 找话题`。
8. 查看行动建议、上下文来源、截图理解、记忆候选和提醒候选。
9. 只在确认无误后复制候选；需要长期保存的记忆或提醒必须手动确认。

## 本地数据与日志

默认本地数据目录：

```text
Linux/macOS: ~/.echomate/
Windows:     %APPDATA%\EchoMate\
E2E mock:   $ECHOMATE_E2E_PROFILE_DIR/
```

常见内容：

```text
echomate.db          # 本地 SQLite/SQLCipher 数据库
config.json          # 本地配置
logs/                # 应用日志（Linux/macOS 默认在 ~/.echomate/logs/）
```

查看当天日志：

```bash
tail -f ~/.echomate/logs/echomate.log.$(date +%Y-%m-%d)
```

macOS 崩溃报告通常在：

```text
~/Library/Logs/DiagnosticReports/echo-mate-*.ips
```

Claude provider 调试输出通常保存在临时目录：

```bash
find /var/folders -path '*echomate-claude*' -type f -name 'last-claude-output.json'
```

更多 macOS 启动、e2e、Claude Code JSON 事件流和快捷键闪退排查经验见 [`docs/macos-e2e-smoke-notes.md`](docs/macos-e2e-smoke-notes.md)。

## 测试

### Rust

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

### 前端脚本语法检查

```bash
node --check frontend/main.js
node --check frontend/settings.js
node --check tests/windows-e2e-runner.mjs
node --check tests/macos-smoke-runner.mjs
```

### macOS 最小端到端 smoke

```bash
npm run test:e2e:macos
```

该命令会使用 `ECHOMATE_E2E_MOCK_PROVIDER=1` 启动临时 Tauri dev 实例，写入剪贴板、点击真实 EchoMate 窗口里的 `生成回复`，并断言候选文本出现。它不依赖 Claude/Codex 外部凭据。

> 测试安全要求：不要把测试写入真实 `~/.echomate/echomate.db`，不要使用真实联系人别名、伴侣上下文或私人聊天截图作为 fixture。运行 Tauri e2e 前应把 `HOME` 或应用数据目录指向临时目录。

## 目录结构

```text
frontend/                         # 静态 HTML/CSS/JS UI
frontend/index.html               # 主窗口：候选、洞察、截图、找话题
frontend/settings.html            # 设置页：provider、隐私、联系人、记忆、提醒、审计
frontend/lib/@tauri-apps/api/     # vendored Tauri JS API，供静态前端直接 import
src-tauri/                        # Rust/Tauri 后端
src-tauri/src/app/                # Tauri 启动、插件、命令注册
src-tauri/src/agent/              # 编排、prompt、schema、输出解析
src-tauri/src/domain/             # 候选、消息、记忆、联系人、提醒等数据模型
src-tauri/src/memory/             # 风格画像、事实抽取和上下文投影
src-tauri/src/platform/           # 热键、剪贴板、截图、输入模拟、macOS 近似上下文
src-tauri/src/provider/           # Claude/Codex CLI provider 与 WSL/process 适配
src-tauri/src/security/           # Keyring 与脱敏工具
src-tauri/src/store/              # SQLite 仓库与迁移
src-tauri/src/ui/                 # Tauri commands、窗口、托盘
src-tauri/capabilities/           # Tauri 权限声明
tests/                            # macOS smoke、Windows e2e、前端 harness
docs/                             # 产品研究、方案、搭建和排查文档
openspec/                         # OpenSpec 配置
.planning/                        # 规划记录
```

## 当前注意点

- EchoMate 仍是本地桌面副驾，不是微信机器人；不会自动发送消息，也不会直接托管真实微信会话。
- Provider 依赖本机 CLI 登录态；如果 `claude` 或 `codex` 未安装/未授权，对应生成会失败或回退到备用 provider。
- 截图生成当前优先使用 Codex 图片输入能力；没有 Codex CLI 或未授权时，截图路径可能失败。
- Claude Code 新版本可能返回 JSON 事件流数组，parser 已兼容 `structured_output` 和 `StructuredOutput` tool input。
- macOS 快捷键复制路径使用 `osascript/System Events` 发送 Cmd+C，避免 enigo 在非主线程访问 macOS 输入源 API 导致闪退。
- SQLCipher 依赖使用 vendored native 构建，首次构建慢是正常现象。
