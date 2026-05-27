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
