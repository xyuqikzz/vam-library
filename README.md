# VAM Library

VAM Library 是一个基于 Tauri 2、Vue 3 和 TypeScript 的桌面端 Virt-A-Mate 资源管理工具。它面向本地 VAM 资源库，帮助用户扫描、浏览、整理、去重、迁移和分享 `.var` 资源包，并为按需启动与 Hub 下载提供基础工作流。

## 当前状态

项目处于早期开发阶段，版本为 `0.1.0`。仓库中已经包含前端界面、Tauri 命令注册、本地 SQLite 索引、资源扫描、依赖分析、迁移、下载等模块的实现或入口。

## 主要功能

- 仪表盘：查看包数量、资源体积、最近资源和健康状态。
- 包管理：浏览本地 `.var` 包，支持搜索、筛选、标签、缩略图和快速删除。
- 场景与外观：按资源类型查看场景、外观预设及其引用关系。
- 依赖分析：查询缺失依赖、反向依赖和包之间的依赖关系。
- 去重分析：扫描重复资源，并将清理项放入应用回收站。
- 资源迁移：按类型、作者、场景或自定义规则整理资源文件。
- 按需启动：将选定主包及其递归依赖映射到运行目录，减少启动时加载范围。
- 在线资源与下载中心：对接 VaM Hub 查询、下载队列和依赖下载流程。
- 一键分享：导出选中资源及其依赖，便于分享或备份。
- 多实例设置：支持保存多个 VAM 实例目录及相关下载、托管配置。

## 技术栈

| 层级 | 技术 |
| --- | --- |
| 桌面框架 | Tauri 2 |
| 前端 | Vue 3、TypeScript、Vite |
| 状态管理 | Pinia |
| 路由 | Vue Router |
| 国际化 | vue-i18n |
| 后端 | Rust |
| 数据库 | SQLite / rusqlite |
| 文件处理 | walkdir、zip、notify |
| 网络下载 | reqwest、tokio |

## 开发环境

建议环境：

- Node.js 18 或更高版本
- npm
- Rust 稳定版工具链
- Tauri 2 所需系统依赖

Windows 环境还需要确保已安装 WebView2 Runtime。Tauri 详细依赖可参考官方文档。

## 安装依赖

```bash
npm install
```

仓库中同时存在 `package-lock.json` 和 `pnpm-lock.yaml`，当前脚本以 `npm` 为默认示例。

## 本地开发

启动完整桌面应用：

```bash
npm run tauri:dev
```

仅启动前端开发服务器：

```bash
npm run dev
```

默认前端开发地址为 `http://localhost:1420`，Tauri 配置会在桌面开发模式下使用该地址。

## 构建

构建前端：

```bash
npm run build
```

构建桌面安装包：

```bash
npm run tauri:build
```

## 常用脚本

| 命令 | 说明 |
| --- | --- |
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run build` | 执行类型检查并构建前端产物 |
| `npm run preview` | 预览前端构建结果 |
| `npm run tauri:dev` | 启动 Tauri 桌面开发模式 |
| `npm run tauri:build` | 构建 Tauri 桌面应用 |

## 项目结构

```text
.
├── public/                 # 静态资源
├── src/                    # Vue 前端代码
│   ├── assets/styles/      # 全局样式与设计变量
│   ├── components/         # 通用组件、布局组件和业务组件
│   ├── composables/        # 可复用组合式逻辑
│   ├── i18n/               # 中英文文案
│   ├── router/             # Vue Router 路由
│   ├── stores/             # Pinia 状态
│   ├── types/              # TypeScript 类型定义
│   └── views/              # 页面视图
├── src-tauri/              # Tauri / Rust 后端
│   ├── capabilities/       # Tauri 权限配置
│   ├── icons/              # 应用图标
│   └── src/
│       ├── commands/       # Tauri IPC 命令
│       ├── db/             # SQLite 初始化和表结构
│       ├── models/         # Rust 数据模型
│       └── services/       # 扫描、下载、解析、监听等服务
├── index.html
├── package.json
├── vite.config.ts
└── README.md
```

## 使用流程

1. 启动应用后，在设置页选择 VAM 安装根目录。
2. 执行扫描，应用会索引 `AddonPackages` 等目录中的资源。
3. 在包管理、场景、外观、依赖分析等页面查看资源状态。
4. 根据需要进行去重、迁移、按需启动、下载或分享操作。

## 数据与文件安全说明

- 本地索引用 SQLite 保存，用于缓存扫描结果、依赖关系、标签、迁移记录和下载状态。
- 清空本地数据只会清除应用数据库，不会删除真实 VAM 资源文件。
- 删除和去重清理会优先进入 VAM Library 回收站，回收站内资源会按应用策略自动清理。
- 资源迁移、按需启动和下载操作会涉及真实文件移动、复制或链接映射，执行前请确认预览内容。

## 开发注意事项

- 前端使用 Vue 3 `<script setup>` 和 TypeScript。
- Tauri 命令统一在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中注册。
- 新增跨端能力时，需要同时关注 Rust 命令、前端 `invoke` 调用和 Tauri 权限配置。
- 大型 VAM 目录可能包含上百 GB 资源和大量 `.var` 文件，扫描、去重和迁移功能需要避免阻塞 UI。

## 许可证

当前仓库暂未声明许可证。如需公开分发，请先补充明确的 License 文件。
