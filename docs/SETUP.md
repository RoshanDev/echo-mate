# EchoMate 本地开发环境搭建指南

> 记录日期: 2026-05-28 | 环境: macOS 12.7.6 (Monterey) · Intel x86_64

## 前置检查

### 已具备的环境

| 组件 | 版本 | 说明 |
|------|------|------|
| macOS | 12.7.6 Monterey | 满足 Tauri 2 最低要求 (macOS 10.15+) |
| Xcode CLT | 已安装 | `xcode-select -p` → `/Library/Developer/CommandLineTools` |
| Homebrew | 5.0.13 | 包管理器，已预装 `openssl@3` |
| Node.js | v22.17.1 | 通过 nvm 管理，用于 Tauri CLI npm 安装 |
| npm | 10.9.2 | Node 包管理器 |
| Claude CLI | 2.1.152 | 已安装在 `~/.local/bin/claude` |

### 需要安装的组件

| 组件 | 安装方式 | 预计耗时 |
|------|---------|---------|
| Rust 工具链 | rustup | ~5 min |
| Tauri CLI | npm (预编译) | ~10s |

## 安装步骤

### 1. 安装 Rust 工具链

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
```

安装完成后加载环境:
```bash
source "$HOME/.cargo/env"
```

验证安装:
```bash
rustc --version   # rustc 1.95.0
cargo --version   # cargo 1.95.0
```

默认 target: `x86_64-apple-darwin`

### 2. 安装 Tauri CLI

npm 方式（推荐，下载预编译二进制，速度快）:
```bash
npm install -g @tauri-apps/cli@latest
```

验证:
```bash
npx tauri --version   # tauri-cli 2.11.2
```

> 备选: `cargo install tauri-cli --version "^2"` 从源码编译，耗时约 15-30 分钟。

### 3. 初始化项目

#### 3.1 创建前端壳

Tauri 需要一个前端目录。在项目根目录创建 `frontend/`:
```
frontend/
  index.html    - 主页面（弹出窗口 UI）
  styles.css    - 样式
  main.js       - 前端逻辑（Tauri API 调用）
```

#### 3.2 使用 Tauri CLI 初始化

```bash
npx tauri init \
  --app-name "EchoMate" \
  --window-title "EchoMate" \
  --frontend-dist "../frontend" \
  --dev-url "http://localhost:1420" \
  --before-dev-command "" \
  --before-build-command "" \
  --ci
```

这会生成 `src-tauri/` 目录，包含:
- `Cargo.toml` - Rust 依赖配置
- `tauri.conf.json` - Tauri 应用配置
- `src/main.rs` - 程序入口
- `src/lib.rs` - 库入口
- `capabilities/` - 权限配置
- `icons/` - 应用图标

#### 3.3 配置依赖

编辑 `src-tauri/Cargo.toml`，添加项目依赖:

```toml
[dependencies]
# Tauri & plugins
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-log = "2"
tauri-plugin-global-shortcut = "2"
tauri-plugin-clipboard-manager = "2"
tauri-plugin-shell = "2"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Async runtime
tokio = { version = "1", features = ["process", "time", "macros", "rt-multi-thread"] }

# Database (encrypted SQLite with SQLCipher)
rusqlite = { version = "0.31", features = ["bundled-sqlcipher-vendored-openssl"] }

# OS keyring for DB key storage
keyring = "3"

# Input simulation (Ctrl/Cmd+C)
enigo = "0.3"

# Logging
tracing = "0.1"
tracing-subscriber = "0.3"

# Date/time
chrono = { version = "0.4", features = ["serde"] }

# Error handling
anyhow = "1"
thiserror = "2"
```

#### 3.4 配置 Tauri 窗口和权限

`tauri.conf.json` 关键配置:
- `identifier`: `com.echomate.app`
- 窗口: 400x420, 置顶, 初始隐藏, 居中
- 系统托盘: 启用
- CSP: 限制为 self

`capabilities/default.json` 权限:
- `core:default` - 核心权限
- `global-shortcut:*` - 全局热键注册
- `clipboard-manager:*` - 剪贴板读写
- `shell:allow-execute` - 外部 CLI 执行
- `log:default` - 日志

### 4. 项目结构

```
echo-mate/
├── frontend/              # 前端 (HTML/CSS/JS 弹出窗口)
│   ├── index.html
│   ├── styles.css
│   └── main.js
├── src-tauri/             # Rust 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/
│   └── src/
│       ├── main.rs        # 程序入口
│       ├── lib.rs         # 模块声明
│       ├── app/           # 启动引导、依赖注入
│       ├── platform/      # 热键、剪贴板、输入模拟
│       ├── agent/         # 编排器、Prompt 组装、JSON 解析
│       ├── provider/      # Claude/Codex CLI 适配器
│       ├── store/         # SQLite 仓库、数据库迁移
│       ├── memory/        # 风格画像、联系人事实、记忆投影
│       ├── security/      # Keyring、脱敏
│       ├── ui/            # Tauri 命令、窗口管理、托盘
│       └── domain/        # 数据模型
├── docs/                  # 设计文档
└── CLAUDE.md              # AI 协作配置
```

### 5. 验证编译

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

首次编译包含 `rusqlite` + `bundled-sqlcipher-vendored-openssl`，需要从源码编译 SQLite + SQLCipher + OpenSSL，预计 15-30 分钟。

后续增量编译约 5 秒。

## 遇到的问题与解决

### 问题 1: Tauri 插件 API 不一致

不同 Tauri 2 插件使用不同的初始化方式:
- `tauri_plugin_log::Builder::new().build()` - 使用 Builder 模式
- `tauri_plugin_global_shortcut::Builder::new().build()` - 使用 Builder 模式
- `tauri_plugin_clipboard_manager::init()` - 使用 init() 函数
- `tauri_plugin_shell::init()` - 使用 init() 函数

解决: 查阅每个插件的源码 API 后使用正确的初始化方式。

### 问题 2: tray-icon feature 未启用

在 `tauri.conf.json` 中配置了 `trayIcon` 但未在 Cargo.toml 的 tauri 依赖中添加 `tray-icon` feature。

解决: `tauri = { version = "2", features = ["tray-icon"] }`

## 技术栈总结

| 层 | 技术 |
|---|---|
| 桌面壳 | Tauri 2 (系统 WebView) |
| 语言 | Rust 1.95.0 |
| 异步运行时 | Tokio |
| 数据库 | SQLite + SQLCipher (加密) |
| 密钥存储 | OS Keyring |
| 热键 | tauri-plugin-global-shortcut |
| 剪贴板 | tauri-plugin-clipboard-manager |
| 输入模拟 | enigo |
| 子进程 | tokio::process |
| 日志 | tracing + tracing-subscriber |
| 前端 | 原生 HTML/CSS/JS |
