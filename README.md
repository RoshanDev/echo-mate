# EchoMate

EchoMate 是一个面向桌面微信工作流的本地 AI 回复副驾。它以 Tauri 2 桌面应用运行，读取剪贴板、选中文本或截图上下文，生成候选回复、找话题建议、记忆候选和提醒候选。

当前仓库不是 HTTP 服务；本地可访问对象是 `EchoMate` 桌面窗口。

## 功能概览

- 基于剪贴板文字生成 5 条候选回复。
- 通过全局快捷键复制当前选中文本并生成回复。
- 通过框选截图把聊天截图作为上下文生成回复。
- 为指定联系人生成低压找话题建议。
- 支持联系人白名单、本地上下文、记忆候选、提醒候选和手动确认保存。
- 设置页可查看、刷新或重置本地风格画像；画像从已采用回复统计为写作规则，不调用外部 provider。
- 支持 Claude CLI / Codex CLI provider；截图输入当前使用 Codex 图片能力。

## 环境要求

- macOS / Windows / Linux 桌面环境。
- Rust 工具链，项目要求 Rust `1.77.2+`。
- Node.js 和 npm。
- Tauri CLI，可通过 `npx tauri ...` 使用。
- 可选：`claude` 或 `codex` CLI，并完成本机登录/授权。

macOS 上热键复制选中文本需要给终端或打包后的应用授予 Accessibility 权限。

## 安装依赖

```bash
npm install
```

Rust 依赖由 Cargo 在首次构建时拉取。

## 启动

macOS / 日常开发推荐：

```bash
npx tauri dev
```

也可以使用 Makefile 的基础入口：

```bash
make check
make build
```

注意：直接运行 `src-tauri/target/debug/echo-mate` 可能缺少 Tauri dev 资源上下文；开发验证优先使用 `npx tauri dev`。

## 使用方式

1. 启动 EchoMate。
2. 在设置里选择 provider、热键和当前联系人。
3. 复制一段聊天内容，点击 `生成回复`。
4. 或选中文本后按全局热键，默认是 `CmdOrCtrl+Shift+Space`，本地配置可能已改为其他组合。
5. 点击 `框选截图` 可用截图上下文生成回复。
6. 点击 `找话题` 可基于当前联系人和本地上下文生成主动开启话题建议。
7. 在设置页的 `风格画像` 区域可查看当前本地写作规则，`刷新画像` 会从已采用回复重新统计。

EchoMate 只生成候选并复制，不会自动发送消息。

## 构建与测试

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

前端脚本语法检查：

```bash
node --check frontend/main.js
node --check frontend/settings.js
node --check tests/windows-e2e-runner.mjs
node --check tests/macos-smoke-runner.mjs
```

macOS 最小端到端 smoke：

```bash
npm run test:e2e:macos
```

这个命令会使用 `ECHOMATE_E2E_MOCK_PROVIDER=1` 启动临时 Tauri dev 实例，写入剪贴板、点击真实 EchoMate 窗口里的 `生成回复`，并断言候选文本出现。它不依赖 Claude/Codex 外部凭据。

## 日志与排查

应用日志：

```bash
tail -f ~/.echomate/logs/echomate.log.$(date +%Y-%m-%d)
```

macOS 崩溃报告：

```text
~/Library/Logs/DiagnosticReports/echo-mate-*.ips
```

Claude provider 调试输出通常保存在临时目录：

```bash
find /var/folders -path '*echomate-claude*' -type f -name 'last-claude-output.json'
```

更多 macOS 启动、e2e、Claude Code JSON 事件流和快捷键闪退排查经验见 [docs/macos-e2e-smoke-notes.md](docs/macos-e2e-smoke-notes.md)。

## 目录结构

```text
frontend/              # HTML/CSS/JS UI
src-tauri/             # Rust/Tauri 后端
src-tauri/src/agent/   # 编排、prompt、schema、parser
src-tauri/src/provider/# Claude/Codex provider
src-tauri/src/platform/# 热键、剪贴板、截图、输入模拟
src-tauri/src/store/   # SQLite 本地存储
tests/                 # macOS smoke、Windows e2e、前端 harness
docs/                  # 设计和排查文档
```

## 当前注意点

- Claude Code 新版本可能返回 JSON 事件流数组，parser 已兼容 `structured_output` 和 `StructuredOutput` tool input。
- macOS 快捷键复制路径使用 `osascript/System Events` 发送 Cmd+C，避免 enigo 在非主线程访问 macOS 输入源 API 导致闪退。
- 截图生成会自动使用 Codex 图片输入能力；如果没有 Codex CLI 或未授权，截图路径可能失败。
