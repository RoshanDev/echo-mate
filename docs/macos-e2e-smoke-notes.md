# macOS E2E Smoke 验证经验

记录日期：2026-06-09

## 项目启动判断

EchoMate 当前是 Tauri 2 桌面应用，不是 HTTP 服务。macOS 本地可访问对象是 EchoMate 桌面窗口；日常启动入口是：

```bash
npx tauri dev
```

构建和基础测试入口：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
```

## 可重复 smoke 路线

新增的 macOS smoke runner 使用确定性的 mock provider，避免依赖本机 Claude/Codex 外部凭据：

```bash
npm run test:e2e:macos
```

覆盖的核心用户流程：

1. 从零启动 `npx tauri dev`。
2. 等待 `echo-mate` 进程和 `EchoMate` 窗口出现。
3. 写入剪贴板文本 `我明天面试，有点紧张`。
4. 通过 macOS Accessibility 点击窗口里的 `⚡ 生成回复` 按钮。
5. 断言 Accessibility 文本中出现 `已生成 5 条候选回复`、`候选 1` 和首条候选回复。
6. 保存前后截图并清理 runner 自己启动的 Tauri 进程。

## 关键经验

- 直接运行 `src-tauri/target/debug/echo-mate` 可能缺少 Tauri dev 资源上下文；macOS 开发验证应优先使用 `npx tauri dev`。
- runner 为了隔离 EchoMate 应用数据会把 `HOME` 指向临时目录，但这样会让 `rustup` 找不到默认工具链；需要显式保留真实 `RUSTUP_HOME` 和 `CARGO_HOME`。
- `System Events` 在当前 macOS 环境偶尔需要 5 秒以上才返回，进程存在性检查更适合用 `pgrep -x echo-mate`，Accessibility 只用于窗口、按钮和文本断言。
- Chrome headless 跑前端 harness 时会输出 `CVDisplayLinkCreateWithCGDisplay failed` 噪声，但退出码为 0 且 `#result` 为 `PASS` 时可视为通过。
- `ECHOMATE_E2E_MOCK_PROVIDER=1` 只用于测试进程，生产启动不会默认启用。
- e2e runner 会恢复原剪贴板文本，并只终止自己启动的 Tauri dev 进程；如果要测试已运行的应用，需要显式设置 `ECHOMATE_E2E_USE_RUNNING=1`。

## Claude Code 输出解析经验

现象：

```text
Claude 生成失败。 技术信息：Failed to parse CandidateEnvelope. Keys: []. result(first 300):
```

结论：这不是 Claude CLI 没配置。日志显示 Claude Code 可以正常启动并返回大量 JSON 输出；失败点是 EchoMate 旧 parser 只接受单个 JSON 对象，而当前 Claude Code `--output-format json` 会返回 JSON 事件流数组。

排查方式：

```bash
tail -n 240 ~/.echomate/logs/echomate.log.$(date +%Y-%m-%d)
find /var/folders -path '*echomate-claude*' -type f -name 'last-claude-output.json'
```

在 `last-claude-output.json` 里，真正可用的结构化回复可能出现在：

- 顶层对象的 `structured_output`
- 顶层对象的 `result`
- Claude Code 事件流数组末尾的 `structured_output`
- assistant message 里的 `StructuredOutput` tool input

修复策略：parser 应先把 stdout 解析为 `serde_json::Value`，再递归识别上述包裹层；如果顶层是数组，从后往前找最终结果，优先使用最后的 `structured_output`。

## macOS 快捷键闪退经验

现象：按全局快捷键后应用闪退；崩溃报告位于：

```text
~/Library/Logs/DiagnosticReports/echo-mate-*.ips
```

关键栈：

```text
dispatch_assert_queue_fail
TSMGetInputSourceProperty
enigo::platform::macos_impl::keycode_to_string
app_lib::platform::input::InputSimulator::copy_selection
```

结论：热键触发任务运行在 Tokio worker 线程上，`enigo` 的 macOS 键盘路径会访问必须在主 dispatch queue 上运行的输入源 API，导致 `SIGILL/EXC_BAD_INSTRUCTION`。这不是用户权限缺失；日志里也可能出现 `The application has the permission to simulate input`，但随后仍会崩。

修复策略：macOS 上不要用 enigo 模拟 Cmd+C；改用 `osascript` 调 System Events：

```applescript
tell application "System Events" to keystroke "c" using command down
```

Windows 仍可保留 enigo 路径。修复后，用 TextEdit 选中文本并按当前配置的全局热键，日志应出现：

```text
Hotkey released, triggering: CmdOrCtrl+Shift+X
Copied selected text length: 30
Clipboard text length: 30
```

## 已验证结果

本轮在 macOS 上验证通过：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo build --manifest-path src-tauri/Cargo.toml
node --check frontend/main.js
node --check frontend/settings.js
node --check tests/windows-e2e-runner.mjs
node --check tests/macos-smoke-runner.mjs
npm run test:e2e:macos
```

后续又验证了：

```bash
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
npm run test:e2e:macos
```

`enigo::Key::Command` 弃用 warning 已通过 macOS 路径移除解决；`cargo build` 当前无 warning。
