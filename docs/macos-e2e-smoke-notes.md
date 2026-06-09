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

剩余注意点：Rust 当前仍有一个既有 warning，`enigo::Key::Command` 已弃用但不影响本次构建、启动和 e2e。
