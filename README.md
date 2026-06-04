# VAM Library

> A cross-platform desktop manager for your local **Virt-A-Mate (VAM)** resource library, built with **Tauri 2 + Vue 3 + TypeScript**.
>
> 一个基于 **Tauri 2 + Vue 3 + TypeScript** 的桌面端 **Virt-A-Mate (VAM)** 本地资源库管理工具。

**Languages / 语言:** [English](#english) · [中文](#中文)

> ⚠️ **Status / 项目状态:** Early development (`v0.1.0`). Features and APIs may change. / 早期开发阶段（`v0.1.0`），功能与接口可能变动。

---

## English

### Overview

VAM Library is a desktop application that helps you manage a local Virt-A-Mate library of `.var` packages. It scans and indexes your resources into a local SQLite database, then provides tools to browse, analyze dependencies, deduplicate, migrate, unpack, launch VAM on demand, and download/share content — all from a single bilingual (English / 中文) interface.

The app uses a Vue 3 frontend talking to a Rust backend via Tauri commands. All indexing data is stored locally; your real VAM files are only touched by the explicit move / copy / link / delete operations you trigger.

### Features

- **Dashboard** — Overview of total packages, library size, recent additions, library health, and detection of corrupted packages.
- **Package management** — Browse indexed `.var` packages with search, folder/tag filtering, thumbnails and scene previews; quick-delete packages, manage tags, and open packages or paths in the system file explorer.
- **Scenes & Appearances** — Dedicated views to browse scenes and appearance presets contained in your library.
- **Statistics** — Aggregated statistics about your resource library.
- **Dependency completion** — Inspect the dependency graph, per-package dependency relations, reverse dependencies, and find missing dependencies.
- **Deduplication** — Scan for duplicate resources, review duplicate groups, preview cleanup, and execute cleanup safely into an in-app recycle bin.
- **Migration** — Preview and execute reorganization of resource files (by type / author / scene / custom rules), with rollback of individual or all migrations, and scene dependency collection.
- **On-demand launch** — Define launch plans that map selected main packages plus their recursive dependencies into the run directory to reduce VAM's load footprint; migrate/restore the library and launch VAM directly or via config.
- **Smart unpack** — Analyze archives and unpack their contents into the library.
- **Online Hub** — Browse the VaM Hub, view package info, check login status, and queue Hub files for download.
- **Download center** — A background download queue with pause / resume / cancel / retry, completed-item cleanup, configurable download settings, and automatic Hub dependency resolution.
- **One-click sharing** — Preview and export selected resources (and their dependencies) as a ZIP, or export the list of installed packages.
- **VAM preferences** — Read and edit VAM's own preference files from within the app.
- **Recycle bin** — Cleanup/deleted items go to an in-app trash with restore, delete, and empty actions; expired trash is auto-purged on startup.
- **Multi-instance settings** — Save multiple VAM instance directories along with download and hosting configuration, Hub auth cookie, and the option to clear the local database.
- **Live file watching** — Optional file-system watcher keeps the index in sync with on-disk changes.
- **Bilingual UI** — English (`en-US`) and Simplified Chinese (`zh-CN`), with the choice persisted locally (defaults to Chinese).

### Tech stack

| Layer | Technology |
| --- | --- |
| Desktop shell | Tauri 2 |
| Frontend | Vue 3, TypeScript, Vite |
| State management | Pinia |
| Routing | Vue Router (hash history) |
| Internationalization | vue-i18n (`zh-CN`, `en-US`) |
| Backend | Rust |
| Database | SQLite via `rusqlite` (bundled) |
| Archives & hashing | `zip`, `sha2`, `md5`, `hex` |
| Filesystem | `walkdir`, `notify` |
| Networking | `reqwest`, `tokio`, `futures` |
| Misc | `chrono`, `regex`, `serde` / `serde_json`, `log`, `thiserror` |

### Requirements

- Node.js 18+
- npm (a `pnpm-lock.yaml` is also present, but scripts below assume npm)
- Rust stable toolchain
- System dependencies required by Tauri 2 (see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))
- On Windows: WebView2 Runtime

### Getting started

```bash
# install dependencies
npm install

# run the full desktop app in dev mode
npm run tauri:dev

# or run only the frontend dev server (http://localhost:1420)
npm run dev
```

### Build

```bash
# build the frontend
npm run build

# build the desktop installers
npm run tauri:build
```

### Scripts

| Command | Description |
| --- | --- |
| `npm run dev` | Start the Vite dev server |
| `npm run build` | Type-check and build the frontend |
| `npm run preview` | Preview the built frontend |
| `npm run tauri:dev` | Run the Tauri desktop app in dev mode |
| `npm run tauri:build` | Build the Tauri desktop app |

### Project structure

```text
.
├── public/                  # Static assets
├── src/                     # Vue frontend
│   ├── assets/              # Global styles & design tokens
│   ├── components/          # Layout & shared components
│   ├── composables/         # Reusable composition logic
│   ├── i18n/                # zh-CN / en-US locale messages
│   ├── router/              # Vue Router routes
│   ├── stores/              # Pinia stores
│   ├── types/               # TypeScript types
│   └── views/               # Page views (dashboard, packages, …)
├── src-tauri/               # Tauri / Rust backend
│   ├── capabilities/        # Tauri permission config
│   ├── icons/               # App icons
│   └── src/
│       ├── commands/        # Tauri IPC commands (scan, packages, dependency, …)
│       ├── db/              # SQLite init & schema
│       ├── models/          # Rust data models
│       └── services/        # scanner, var_parser, watcher, downloader, install_context
├── index.html
├── package.json
├── vite.config.ts
└── README.md
```

### Typical workflow

1. On first launch, open **Settings** and select your VAM installation root directory.
2. Run a **scan** — the app indexes resources under `AddonPackages` and related folders into the local database.
3. Browse and inspect resources in the Packages, Scenes, Appearances and Dependency views.
4. Use Deduplication, Migration, On-demand launch, Smart unpack, Download or Share as needed.

### Data & file safety

- The local SQLite index caches scan results, dependency relations, tags, migration records and download state.
- Clearing local data only wipes the app database — it never deletes your real VAM resource files.
- Deletions and deduplication cleanups go to the VAM Library recycle bin first; expired trash is purged automatically per app policy.
- Migration, on-demand launch, unpack and download operations move/copy/link real files — always confirm the preview before executing.

### Contributing

- The frontend uses Vue 3 `<script setup>` with TypeScript.
- All Tauri commands are registered in the `invoke_handler` in `src-tauri/src/lib.rs`.
- When adding a cross-boundary capability, update the Rust command, the frontend `invoke` call, and the Tauri permission config together.
- Large VAM libraries can contain hundreds of GB and many `.var` files — keep scanning, dedup and migration off the UI thread to avoid blocking.

### License

Released under the [MIT License](./LICENSE). Copyright (c) 2026 xyuqikzz.

> VAM Library is an unofficial, third-party tool and is not affiliated with or endorsed by the creators of Virt-A-Mate.

---

## 中文

### 项目简介

VAM Library 是一个用于管理本地 Virt-A-Mate 资源库（`.var` 资源包）的桌面应用。它会扫描并将资源索引到本地 SQLite 数据库，然后提供浏览、依赖分析、去重、迁移、解包、按需启动、下载与分享等一系列功能——全部集成在一个中英双语界面中。

应用采用 Vue 3 前端 + Rust 后端，通过 Tauri 命令通信。所有索引数据保存在本地；只有你主动触发的移动 / 复制 / 链接 / 删除操作才会动到你真实的 VAM 文件。

### 主要功能

- **仪表盘** — 总览包数量、资源体积、最近新增、库健康状态，并检测损坏的资源包。
- **包管理** — 浏览已索引的 `.var` 包，支持搜索、文件夹 / 标签筛选、缩略图与场景预览；可快速删除、管理标签，并在系统文件管理器中打开包或路径。
- **场景与外观** — 专门的视图用于浏览库中的场景与外观预设。
- **统计** — 资源库的聚合统计信息。
- **依赖补全** — 查看依赖关系图、单包依赖关系、反向依赖，并查找缺失的依赖。
- **去重分析** — 扫描重复资源，查看重复组，预览清理方案，并将清理项安全地放入应用回收站。
- **资源迁移** — 按类型 / 作者 / 场景 / 自定义规则预览并执行文件重组，支持单次或全部迁移回滚，以及场景依赖收集。
- **按需启动** — 定义启动方案，将选定主包及其递归依赖映射到运行目录，减少 VAM 启动时的加载范围；可迁移 / 恢复资源库，并直接或通过配置启动 VAM。
- **智能解包** — 分析压缩包并将其内容解包到资源库。
- **在线 Hub** — 浏览 VaM Hub、查看包信息、查看登录状态，并将 Hub 文件加入下载队列。
- **下载中心** — 后台下载队列，支持暂停 / 继续 / 取消 / 重试、清理已完成项、可配置下载设置，以及自动解析 Hub 依赖。
- **一键分享** — 预览并导出选定资源（及其依赖）为 ZIP，或导出已安装包列表。
- **VAM 偏好设置** — 在应用内读取和编辑 VAM 自身的偏好设置文件。
- **回收站** — 清理 / 删除项会进入应用回收站，支持恢复、删除和清空；过期回收站在启动时自动清理。
- **多实例设置** — 保存多个 VAM 实例目录以及相关下载、托管配置、Hub 认证 Cookie，并可选择清空本地数据库。
- **实时文件监听** — 可选的文件系统监听器，让索引与磁盘变更保持同步。
- **中英双语界面** — 支持简体中文（`zh-CN`）与英文（`en-US`），选择会本地保存（默认中文）。

### 技术栈

| 层级 | 技术 |
| --- | --- |
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3、TypeScript、Vite |
| 状态管理 | Pinia |
| 路由 | Vue Router（hash 模式） |
| 国际化 | vue-i18n（`zh-CN`、`en-US`） |
| 后端 | Rust |
| 数据库 | SQLite / `rusqlite`（bundled） |
| 压缩与哈希 | `zip`、`sha2`、`md5`、`hex` |
| 文件处理 | `walkdir`、`notify` |
| 网络下载 | `reqwest`、`tokio`、`futures` |
| 其他 | `chrono`、`regex`、`serde` / `serde_json`、`log`、`thiserror` |

### 开发环境

- Node.js 18 或更高版本
- npm（仓库中也存在 `pnpm-lock.yaml`，但下方脚本以 npm 为例）
- Rust 稳定版工具链
- Tauri 2 所需的系统依赖（参见 [Tauri 环境要求](https://v2.tauri.app/start/prerequisites/)）
- Windows 环境需要已安装 WebView2 Runtime

### 快速开始

```bash
# 安装依赖
npm install

# 启动完整桌面应用（开发模式）
npm run tauri:dev

# 或仅启动前端开发服务器（http://localhost:1420）
npm run dev
```

### 构建

```bash
# 构建前端
npm run build

# 构建桌面安装包
npm run tauri:build
```

### 常用脚本

| 命令 | 说明 |
| --- | --- |
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run build` | 执行类型检查并构建前端产物 |
| `npm run preview` | 预览前端构建结果 |
| `npm run tauri:dev` | 启动 Tauri 桌面开发模式 |
| `npm run tauri:build` | 构建 Tauri 桌面应用 |

### 项目结构

```text
.
├── public/                  # 静态资源
├── src/                     # Vue 前端代码
│   ├── assets/              # 全局样式与设计变量
│   ├── components/          # 布局与通用组件
│   ├── composables/         # 可复用组合式逻辑
│   ├── i18n/                # zh-CN / en-US 文案
│   ├── router/              # Vue Router 路由
│   ├── stores/              # Pinia 状态
│   ├── types/               # TypeScript 类型定义
│   └── views/               # 页面视图（仪表盘、包管理等）
├── src-tauri/               # Tauri / Rust 后端
│   ├── capabilities/        # Tauri 权限配置
│   ├── icons/               # 应用图标
│   └── src/
│       ├── commands/        # Tauri IPC 命令（scan、packages、dependency 等）
│       ├── db/              # SQLite 初始化与表结构
│       ├── models/          # Rust 数据模型
│       └── services/        # scanner、var_parser、watcher、downloader、install_context
├── index.html
├── package.json
├── vite.config.ts
└── README.md
```

### 使用流程

1. 首次启动后，在**设置**页选择你的 VAM 安装根目录。
2. 执行**扫描**，应用会索引 `AddonPackages` 等目录中的资源到本地数据库。
3. 在包管理、场景、外观、依赖分析等页面查看资源状态。
4. 根据需要进行去重、迁移、按需启动、智能解包、下载或分享。

### 数据与文件安全说明

- 本地 SQLite 索引用于缓存扫描结果、依赖关系、标签、迁移记录和下载状态。
- 清空本地数据只会清除应用数据库，不会删除真实的 VAM 资源文件。
- 删除和去重清理会优先进入 VAM Library 回收站，回收站内资源会按应用策略自动清理。
- 资源迁移、按需启动、解包和下载操作会涉及真实文件的移动、复制或链接映射，执行前请确认预览内容。

### 参与开发

- 前端使用 Vue 3 `<script setup>` 和 TypeScript。
- 所有 Tauri 命令统一在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中注册。
- 新增跨端能力时，需要同时关注 Rust 命令、前端 `invoke` 调用和 Tauri 权限配置。
- 大型 VAM 目录可能包含上百 GB 资源和大量 `.var` 文件，扫描、去重和迁移功能需要避免阻塞 UI。

### 开源许可

本项目采用 [MIT License](./LICENSE) 开源。Copyright (c) 2026 xyuqikzz。

> VAM Library 是非官方的第三方工具，与 Virt-A-Mate 官方无关。
