# Agent Nexus

本地桌面应用，基于 Tauri 2 + React + TypeScript 构建。

## 前置依赖

| 工具 | 版本要求 |
|------|---------|
| [Node.js](https://nodejs.org/) | ≥ 20 |
| [pnpm](https://pnpm.io/) | 11.6.0（见 `packageManager`）|
| [Rust](https://rustup.rs/) | stable（通过 rustup 安装）|

> macOS 还需安装 Xcode Command Line Tools；Windows 需安装 [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)。
> Windows 下 `pnpm tauri ...` 和 `pnpm rust:test` 会自动下载并校验官方 SQLite DLL，生成 `sqlite3.lib` 后动态链接。

## 安装依赖

```bash
pnpm install
```

## 启动开发环境

```bash
# 完整 Tauri 桌面应用（推荐）
pnpm tauri dev

# 仅启动前端（浏览器预览，不含 Rust 后端）
pnpm dev
```

首次运行 `pnpm tauri dev` 时，Cargo 会编译 Rust 依赖，耗时较长，后续启动会快很多。

前端开发服务器默认运行在 `http://localhost:3000`。

## 构建生产包

```bash
pnpm tauri build
```

产物位于 `src-tauri/target/release/`。

## 其他命令

```bash
# 仅构建前端
pnpm build

# TypeScript 类型检查
pnpm typecheck

# Rust 测试（Windows 会先准备动态 SQLite）
pnpm rust:test
```

## 项目结构

```
agent-nexus/
├── src-react/      # 前端（React + Vite + TypeScript）
├── src-tauri/      # 后端（Rust + Tauri 2 + SQLite）
└── docs/           # 设计文档
```
