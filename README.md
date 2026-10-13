# VAM Library

> A cross-platform desktop manager for your local **Virt-A-Mate (VAM)** resource library, built with **Tauri 2 + Vue 3 + TypeScript**.
>
> 一个基于 **Tauri 2 + Vue 3 + TypeScript** 的桌面端 **Virt-A-Mate (VAM)** 本地资源库管理工具。

**Languages / 语言:** [English](#english) · [中文](#中文)

> ⚠️ **Status / 项目状态:** Early development (`v0.1.3`). Features and APIs may change. / 早期开发阶段（`v0.1.3`），功能与接口可能变动。

---

## English

### Overview

VAM Library is a desktop application that helps you manage a local Virt-A-Mate library of `.var` packages. It scans and indexes your resources into a local SQLite database, then provides tools to browse, analyze dependencies, deduplicate, migrate, import, and download/share content — all from a single bilingual (English / 中文) interface.

The app uses a Vue 3 frontend talking to a Rust backend via Tauri commands. All indexing data is stored locally; your real VAM files are only touched by the explicit move / copy / link / delete operations you trigger.

### Features

- **Presets and game favorites** — The Presets page offers Appearance (default), Clothing, Hair, Morph, Skin, Plugin, Animation and Pose tabs, listing individual VAR and local `.vap` files with thumbnails and read-only character, clothing, hair, morph and JSON details. Copy all indexed VAR presets of the selected type (or one preset) into `Custom/Atom/Person/<type directory>/VAM Library/<package>/<original subfolders>`. Animation uses `AnimationPresets`; each type is isolated for listing, copying and package view. Existing identical files are skipped; different contents are reported without overwriting. Package-local asset references are qualified to the source VAR, which must remain available with its dependencies. The Copy to local presets button asks for confirmation and explains the selected-type scope before writing. Local preset names omit the Preset_ prefix in the UI; renaming xxx writes Preset_xxx.vap together with its thumbnails and favorite/hidden markers; external references to their old filenames are not rewritten. The Scenes page reads native `.json.fav` markers, supports adding/removing favorites, searching and per-instance display names, and includes orphan favorites for removal. Display names apply only in this app. Refresh or return to the app to pick up changes made in the game; reopen the game browser to refresh its view. The original package views remain available in both pages.

- **In-game scene browser mod** — Install/uninstall the bundled BepInEx mod from Settings. Adds author A–Z grouping and physical `AddonPackages` folder navigation to VaM's scene browser. Targets the recorded VaM 1.22.0.13 / BepInEx build; see [mod instructions](mods/SceneBrowser/README.md).

- **Dashboard** — Overview of total packages, library size, recent additions, library health, and detection of corrupted packages.
- **Package management** — Browse indexed `.var` packages with search, folder/tag filtering, thumbnails and scene previews; select a scene in package details and launch directly into it through the bundled scene browser mod (1.0.4+), with desktop/OpenVR modes and support for an already running game. Quick-delete packages, manage tags, and open packages or paths in the system file explorer.
- **Scenes & Appearances** — Dedicated views to browse scenes and appearance presets contained in your library.
- **Statistics** — Aggregated statistics about your resource library.
- **Dependency completion** — Inspect the dependency graph, per-package dependency relations, reverse dependencies, and find missing dependencies.
- **Deduplication** — Scan for duplicate resources, review duplicate groups, preview cleanup, and execute cleanup safely into an in-app recycle bin.
- **Migration** — Preview and execute reorganization of resource files (by type / author / scene / custom rules), with rollback of individual or all migrations, and scene dependency collection.
- **Quick import** — Recursively preview and move `.var` packages from a selected folder into the game's `AddonPackages`. Compare source and installed packages by creator/name, keep the highest numeric version, and break version ties by modification time closest to now (exact ties keep the installed copy). Choose flat, type, creator, scene or a custom relative folder. Duplicates and superseded versions go to the recycle bin; successful groups are indexed immediately. Non-VAR files stay in place. Corrupt winning packages and managed mappings are reported and skipped. Scenes pinned to an older version may require restoring that dependency from the recycle bin.
- **Drop VAR files onto the EXE** — Drop one or more `.var` files onto the application executable or its shortcut, including while the app is running. Confirm whether to import, choose the `AddonPackages` root or folders by resource type, then review the import preview before confirming the move. Only the dropped files are selected; neighboring files are untouched. Cancel preserves the originals. Configure the game directory first if prompted.
- **Online Hub** — Browse the VaM Hub, view package info, check login status, and queue Hub files for download.
- **Download center** — A background download queue with pause / resume / cancel / retry, completed-item cleanup, configurable download settings, and automatic Hub dependency resolution.
- **One-click sharing** — Preview and export selected resources (and their dependencies) as a ZIP, or export the list of installed packages.
- **VAM preferences** — Read and edit VAM's own preference files from within the app.
- **Recycle bin** — Cleanup/deleted items go to an in-app trash with restore, delete, and empty actions; batch retention defaults to 30 days with automatic permanent deletion off. Optional startup purging applies only to expired, completed batches in the active instance; legacy single entries remain for manual review.
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
4. Use Deduplication, Migration, Quick import, Download or Share as needed.

### Batch resource cleanup

- Open **Resource cleanup** to browse, search, filter, select and preview packages in one place. All indexed resources are shown by default, largest first; marking packages first is optional. Scan directly from this page when the index is empty or out of date. Optional pending/protection marks and select-all-filtered-results are under **More actions**. Select-this-page and select-all-filtered-results are distinct snapshots; future imports never join an existing selection. Marks are scoped to the canonical game instance and survive clearing the index.
- **Standalone cleanup** finds packages with no dependencies on other packages and no use by packages, local files, favorites or activation plans. It excludes packages containing scenes by default. Toggle multiple resource types to exclude any package containing one of those types; clear all exclusions to check every type. Missing dependencies, unreadable references and manually protected packages are excluded. Review the usual cleanup preview before moving anything to the recycle bin.
- **Resource cleanup** also includes the existing deduplication tool and cleanup history. Preview whole VAR packages and every physical copy. Include independent dependencies only when requested; keep individual candidates in the preview to recalculate their dependency closure.
- Checks cover VAR metadata and serialized JSON/VAP/VAJ references, local `Custom`/`Saves` files, game favorite markers, saved activation plans, fixed numeric versions and `latest` targets. Shared dependencies and protected resources remain by default. If a selected package is still used or references cannot be confirmed, choose **Remove my selected packages anyway**, review known uses and confirm the move. This applies only to explicitly selected packages; automatic attached dependencies remain when any check is incomplete or quick mode is used. Manual protection must be removed first, and mapping/shared-directory safety cannot be overridden. Runtime-generated plugin references cannot be inferred; manually protect resources you need. Managed mappings must first be restored to ordinary directories; directories shared by other configured instances must be separated before cleanup.
- Reference results are saved per game instance in SQLite and reused after restarting the app. The first cleanup builds missing entries; subsequent previews only read new, changed, or uncached files. A quick directory/file-state check still detects new consumers. Filesystem change stamps detect replacements and edits even when size and modification time are preserved; unsupported filesystems fall back to ZIP directory signatures or local-file hashes. Successful partial indexing is retained after cancellation. Renamed VARs (including copy suffixes such as `.1_2`) are inspected as reference sources without changing their filenames. Archives that cannot be opened at all (HTML/XML download error pages, other archive formats renamed to .var, ZIPs whose directory cannot be read) are reported as problem packages: they can neither use nor be used by anything, so they never block other cleanup and can be removed without an override. Unreadable entries inside an archive and unreadable local files remain uncertain. Previews only report what concerns the selected packages; library-wide format notes are logged instead. A package is never reported as a user of itself. Execution requires acknowledgment for packages the user chose to remove despite known or uncertain use, and for quick checks, then rechecks the live inventory and references before moving files. Restoring a malformed package restores its bytes and prior index record without pretending it passed a new scan.
- Execution requires the corresponding VaM instance to be closed and revalidates the preview. File moves, rollback and restoration use durable per-instance batch journals. Restore keeps newer indexed tags/import times and never overwrites different contents. Interrupted cleanup is reconciled on startup; unresolved batches remain for recovery.
- The recycle bin offers batch restore, a 30-day extension and explicit permanent deletion. Defaults: 30-day suggested retention, automatic permanent deletion off. Opt-in startup purging touches only expired, completed batches in the active instance. Legacy single entries are never automatically purged. Moving to trash on the same drive does not free disk space.

### Recursive deduplication and flattening

- Scan all regular files directly from disk, including unindexed packages. Group VAR files by creator and resource name; keep the highest numeric version, then the most recently modified copy.
- Preserve explicitly referenced versions from `meta.json` and archive older versions under `依赖旧版本/`. `latest` references do not pin old versions. A pinned version needs one surviving copy. If dependency metadata cannot be read, retain all versions until repaired; same-version copies in an affected group can still be recycled when SHA-256 confirms identical contents, keeping the most recently modified copy. Different contents remain protected and are reported as conflicts.
- Removing identical copies checks the file inventory and relevant archive hashes without parsing unrelated scene or preset contents. Removing an entire package version still requires the full reference check.
- Other files require both matching names and SHA-256 contents. Cleanup uses the recycle bin and **manual review for legacy entries**; automatic deletion is disabled by default for new cleanup batches. Archived dependencies are not recycled.
- Deduplicate first, then use **Flatten to root → real AddonPackages → Move**. All regular files are included; the dependency archive remains a separate folder. Conflicting VAR names stay in place; other name collisions receive numeric suffixes.
- Only empty source subfolders are removed after moves. Copies keep source folders. Links, known managed mappings and internal download directories are excluded. Execution revalidates its saved preview; migrations and dependency archives are recorded for rollback.

### Download failures and saved-file warnings

- Temporary connection failures and HTTP 408/429/500/502/503/504 responses receive up to three attempts with bounded backoff; 401/403/404 produce actionable errors. Partial files survive connection failures, and resume responses must match the requested byte range.
- A download is validated as a complete ZIP with readable entries and CRC checks. Invalid `meta.json` does not delete an otherwise intact archive: the download completes with a persistent warning and a basic index without dependency metadata. This does not repair the author's metadata or guarantee that VaM can load it.
- Database indexing failure preserves the downloaded file. **Retry indexing** reuses that file without downloading it again. Existing installed files are not silently overwritten.

### Data & file safety

- The local SQLite index caches scan results, dependency relations, tags, migration records and download state.
- Clearing local data only wipes the app database — it never deletes your real VAM resource files.
- Deletions and deduplication cleanups go to the VAM Library recycle bin first; expired trash is purged automatically per app policy.
- Migration, quick import and download operations move/copy real files — always confirm the preview before executing.

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

VAM Library 是一个用于管理本地 Virt-A-Mate 资源库（`.var` 资源包）的桌面应用。它会扫描并将资源索引到本地 SQLite 数据库，然后提供浏览、依赖分析、去重、迁移、快捷入库、下载与分享等一系列功能——全部集成在一个中英双语界面中。

应用采用 Vue 3 前端 + Rust 后端，通过 Tauri 命令通信。所有索引数据保存在本地；只有你主动触发的移动 / 复制 / 链接 / 删除操作才会动到你真实的 VAM 文件。

### 主要功能

- **VaM 管理增强插件** — 在设置中安装 / 更新 / 卸载，支持作者、入库时间和资源包大小排序，`AddonPackages` 文件夹浏览与场景启动联动。记住排序，支持中英文；入库、扫描和刷新资源时自动同步入库时间。详见[插件说明](mods/SceneBrowser/README.md)。

- **仪表盘** — 总览包数量、资源体积、最近新增、库健康状态，并检测损坏的资源包。
- **包管理** — 浏览已索引的 `.var` 包，支持搜索、文件夹 / 标签筛选、缩略图与场景预览；在包详情中选择场景，点击「启动场景」，通过内置 VaM 管理增强插件（1.2.0）打开游戏并加载场景。支持桌面 / OpenVR 模式，游戏已运行时直接加载。可快速删除、管理标签，并在系统文件管理器中打开包或路径。
- **场景与外观** — 专门的视图用于浏览库中的场景与外观预设。
- **统计** — 资源库的聚合统计信息。
- **依赖补全** — 查看依赖关系图、单包依赖关系、反向依赖，并查找缺失的依赖。
- **去重分析** — 扫描重复资源，查看重复组，预览清理方案，并将清理项安全地放入应用回收站。
- **资源迁移** — 按类型 / 作者 / 场景 / 自定义规则预览并执行文件重组，支持单次或全部迁移回滚，以及场景依赖收集。
- **快捷入库** — 递归预览所选文件夹内的 `.var` 包并移动到游戏 `AddonPackages`。按作者及包名同时比较来源和已安装文件，保留最高数字版本；同版本保留修改时间最接近当前时间的文件，完全相同时保留已安装文件。支持平铺、按类型、按作者、按场景和自定义相对子目录。重复包与旧版本进入回收站，已完成的资源组立即同步索引，普通文件保留原处。损坏的候选保留包及托管映射会报告并跳过。固定引用旧版本的场景可能需要从回收站恢复对应依赖。
- **拖拽 VAR 到 EXE 入库** — 将一个或多个 `.var` 文件拖到软件 EXE 或其快捷方式上，软件已运行时也能接收。弹窗询问是否入库，并提供 `AddonPackages` 根目录和按资源类型分类两种选择；查看入库预览后再确认移动。仅选中拖入的文件，不扫描同目录的其他文件；取消保留原文件。未配置游戏目录时可先进入设置。
- **在线 Hub** — 浏览 VaM Hub、查看包信息、查看登录状态，并将 Hub 文件加入下载队列。
- **下载中心** — 后台下载队列，支持暂停 / 继续 / 取消 / 重试、清理已完成项、可配置下载设置，以及自动解析 Hub 依赖。
- **一键分享** — 预览并导出选定资源（及其依赖）为 ZIP，或导出已安装包列表。
- **VAM 偏好设置** — 在应用内读取和编辑 VAM 自身的偏好设置文件。
- **回收站** — 清理 / 删除项会进入应用回收站，支持恢复、删除和清空；批次默认建议保留 30 天，自动永久删除关闭。开启后仅在启动时检查当前实例的已到期完整批次；历史单项保留供手动处理。
- **多实例设置** — 保存多个 VAM 实例目录以及相关下载、托管配置、Hub 认证 Cookie，并可选择清空本地数据库。
- **实时文件监听** — 可选的文件系统监听器，让索引与磁盘变更保持同步。
- **中英双语界面** — 支持简体中文（`zh-CN`）与英文（`en-US`），选择会本地保存（默认中文）。

### 预设与游戏收藏

- **预设**：顶部依次切换外观预设（默认）、服装预设、头发预设、变形预设、皮肤预设、插件预设、动画预设、姿势预设。逐个查看 VAR 内和游戏本地预设、缩略图及完整只读内容。支持单个复制和一键复制当前类型的全部已索引预设；资源包视图也只显示包含当前类型预设的包。未索引的资源包请先扫描。
- **复制目标**：`游戏目录/Custom/Atom/Person/<类型目录>/VAM Library/<资源包>/<原有子目录>`，类型目录依次是 `Appearance`、`Clothing`、`Hair`、`Morphs`、`Skin`、`Plugins`、`AnimationPresets`、`Pose`。只复制当前选中的类型，已有本地预设无需复制。预览图一并复制，同名同内容跳过、同名不同内容报告冲突；保留原 VAR，复制后的包内引用仍依赖原包及其依赖，游戏中必须启用这些包。
- **复制确认与改名**：点击“复制为本地预设”后，弹窗说明将复制资源包文件夹中当前类型的全部已索引预设，不受搜索和来源筛选影响，确认后才执行。复制后选择“游戏本地文件”，名称只需填写 `xxx`，保存为 `Preset_xxx.vap`，不重复添加 `Preset_`；缩略图和收藏/隐藏标记同步改名。其他文件引用旧名称的路径不自动修改，VAR 内文件不直接改名。
- **收藏夹**：读取游戏真实收藏，默认显示已收藏场景；关闭“只看收藏”可浏览全部场景并添加收藏。支持搜索、添加/移除收藏和编辑软件内显示名称。工具栏等高对齐，列表与详情独立滚动，分页固定在底部；切换资源包视图会保留筛选和图片缓存。移除收藏保留场景原文件；缺失或未索引资源的收藏仍可移除。显示名称按游戏实例隔离，清空即可恢复原名，不改动游戏场景名。
- 切回软件只刷新文件列表和收藏状态，复用已加载的缩略图（包括无预览图的结果）；手动刷新或重新扫描会更新图片。打开详情保持卡片尺寸不变。游戏中的修改可通过“刷新”或切回软件读取；软件修改收藏后，重新打开游戏场景浏览器刷新。测试覆盖复制引用、冲突、改名及标记迁移、收藏往返、失效收藏、路径越界、目录链接和实例隔离。

```powershell
cargo test --manifest-path src-tauri/Cargo.toml game_content
# Optional compatibility checks: game files are read-only; copied outputs use temporary fixtures.
$env:VAM_CONTENT_TEST_ROOT = '<VaM root>'
$env:VAM_CONTENT_TEST_DB = '<app data>/com.vamlibrary.app/vamlibrary.db'
cargo test --manifest-path src-tauri/Cargo.toml game_content -- --include-ignored --skip real_large_scene_extraction_compatibility
# Optional large-scene extraction check, also writing only to a temporary fixture.
$env:VAM_CONTENT_TEST_PACKAGE = '<full path to a VAR package>'
$env:VAM_CONTENT_TEST_SCENE = 'Saves/scene/<scene>.json'
cargo test --manifest-path src-tauri/Cargo.toml real_large_scene_extraction_compatibility -- --ignored
```

- **提取场景角色**：在任意资源列表中点击资源包，展开右侧详情面板的“提取场景角色”，选择包内场景和直接保存的 Person 角色。直接读取所选 VAR，无需依赖资源类型标签或内容索引。填写名称并选择保存目录，默认直接保存为 `Custom/Atom/Person/Appearance/Preset_<名称>.vap`。支持手动编辑完整路径、浏览选择文件夹、恢复默认目录；成功保存后按游戏实例记住目录。预设与缩略图写入同一目录。位于当前游戏外观目录内时，可点击“查看外观预设”定位；保存到其他目录时显示实际路径，需手动加载。保留已保存的基础模型、皮肤、变形、服装、头发及相关材质/物理参数，排除场景位置、控制器动作、动画和插件；同名不覆盖，场景和 VAR 保持不变。VAR 内引用会转换为来源包引用，仍需保留并启用原包及依赖。提取面板显示所选场景的默认预览图，保存时以预设同名复制 JPG/PNG/JPEG 缩略图（保留原格式）；没有预览图时仍可保存预设。该图是场景预览，不是角色独立截图。插件运行时修改或子场景内角色需先在游戏内保存到场景。保存前检查场景是否改变，避免导出旧选择。

### 技术栈

单资源清理：在资源清理页切换到“单资源清理”，查询既不依赖其他资源包，也没有被资源包、本地文件、收藏或启用方案使用的包。默认排除包含场景的包，可同时选中其他要排除的类型；包内包含任一已选类型就会排除，取消全部排除则查询所有类型。缺失依赖、引用检查不完整和手动保护的资源不会列入。选中后照常预览，再移入回收站。

资源清理：在资源包页进入“整理模式”，支持跨页选择、待清理标记、手动保护和作者 / 体积筛选；“选择本页”和“选择全部筛选结果”独立，新下载不会加入已有选择。资源清理页集中显示待清理资源、现有去重工具和批次记录，并提供“有问题”（无效下载、已损坏、缺少依赖）和“可以考虑清理”（没被用到、旧版本、多份副本）筛选；“没被用到”只统计贴图、变形、素材这类依赖包。预览只显示与所选包相关的内容，结论在前，并列出“只被它们用到的依赖”能多腾出的空间。预览针对完整 VAR 与全部物理副本，可选择保留某个包并重新计算依赖。收藏、本地场景 / 预设、启用方案、固定版本与共享依赖默认保留。用户仍想移走某个包时，可在预览里对这个包选择“仍然移走”，或“连同使用它的包一起清理”，确认后移入回收站；只覆盖明确选择的包。引用不完整或快速检查时，自动附带的依赖继续保留；手动保护需先取消，目录映射和共用目录的安全限制仍保留。游戏可接受的非标准 JSON 格式会保守提取引用，完整检查后只记入日志，不在预览中显示，也不阻止清理；缺少 meta.json 的包在完整模式下仍检查全部引用文件。带 `.1_2` 等复制后缀的包仍检查内容引用，不修改原文件名；完全打不开的压缩包（下载失败存下的 HTML/XML 网页、改了扩展名的 7z / RAR、读不出目录的损坏 ZIP）归为问题包，不阻止其他资源清理，选中后无需“仍然移走”即可移入回收站。压缩包内个别文件无法读取、字符串未结束、无效编码或本地文件截断等情况保留不确定提示，停止自动清理依赖，但用户可确认后移走选中包。恢复异常包会恢复原文件；能够解析的非标准名称会补全资源记录，仍无法解析的文件保留之前的检查记录和扫描失败状态。兼容规则升级后旧引用缓存会重新建立一次，后续复用结果。执行前关闭对应游戏，并重新验证预览。取消或失败会尝试回退整个批次，意外退出后保留日志供恢复；恢复不覆盖不同内容，并优先保留已更新的标签和入库时间。回收站默认建议保留 30 天，自动永久删除关闭；开启后仅在应用启动时处理当前实例已到期的完整批次。历史单项不会自动清空，清空索引仍保留整理状态与回收记录。插件运行时生成的引用无法静态推断，请手动保护需要的资源；按需托管映射需先恢复普通目录；多个已配置实例共用的目录需先拆分。

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

### 测试

```bash
# 前端回归：排序、异步状态、订阅清理、Hub 解析及富文本安全（模拟 Tauri IPC）
npm test

# Rust 单元与集成测试；需要本地游戏文件的测试默认忽略
cargo test --locked --manifest-path src-tauri/Cargo.toml
```

前端测试使用 Node.js 22.22.2+（22 LTS 分支，与 CI 一致），也支持 Node.js 24.15+ / 26+；无需启动桌面应用。

Hub 描述保留普通 HTML 和 BBCode，经 DOMPurify 清理后显示；脚本、事件属性、危险链接和页面覆盖样式不会进入界面。jsdom 仅用于测试这一边界，不进入桌面运行包。

去重分析提供两种方式：默认“快速查找”先按文件大小筛选，再用完整 SHA-256 比较所有 VAR，文件名不同也能归组；“仔细查找”进一步按 ZIP 文件目录筛选，流式比较全部包内文件（包含 meta.json），识别重新压缩后的相同内容，不生成解压临时文件。整包已确认相同的副本只需选一个代表读取包内内容。与旧版本整理合并为一份预览，每个文件仅出现一次；同名内容冲突会保留，独立名称的引用和手动保护不会被内容相同绕过。

两种方式共享按资源目录保存的检查缓存，通过文件身份和可靠变更标记失效；没有可靠标记时重新检查。界面使用左侧分组、右侧文件对比，区分可清理和需要保留的资源，清理前仍需核对清单；支持停止查找，并显示查找与整理的实际阶段进度。文件内容校验按文件顺序执行以限制磁盘占用；深度比较限制条目数和展开大小，异常或重复的包内路径不会成为相同内容的证据。执行前只复核本次涉及的文件及保留副本，Windows 下持有阻止写入的文件句柄直至操作完成；确认文件身份、可靠变更标记与分析时一致后复用内容比较结果，避免整理时再次完整读取或解压。文件变化、缺少可靠标记或无法阻止写入的平台仍完整复核；跨名称清理仍进行完整引用检查，支持回收站恢复。

文件名按第一个点拆作者、最后一个点拆版本，中间保留为完整名称。`.1_2`、`.1 (2)`、`.v3`、`.latest` 或没有版本的名称会继续尝试读取包内容，保留原文件名和完整标识；未知版本不参与数字版本匹配，仍提示文件名异常。`.1`、`.01` 和 `.1_2` 不会合并成同一个包。更新后重新扫描即可补全原先因名称异常而缺失的资源记录。

扫描需要完整读取目录清单，目录读取失败时保留已有索引并报告错误；大资源库不再受单条清理 SQL 的参数数量限制。设置采用完整文件替换写入，损坏的 JSON 会报告错误，不会在修改实例或登录信息时静默重置。

开发模式的后端 `[perf]` 日志会分别记录依赖任务等待、数据库等锁、快照读取和匹配耗时，以及缩略图路径查询、缓存读取和图片提取耗时；前端 `[perf:ipc]` 是包含 IPC 往返的总耗时。依赖分析按事务版本共享结果，扫描、入库和删除提交后自动失效。扫描保留未变化文件的图片缓存，文件身份或变更时间改变时重新提取；无法取得可靠变更标记时不复用磁盘缓存，显式清空缓存仍然有效。

可用只读基准检查实际资源库的匹配结果与耗时（PowerShell）：

```powershell
$env:VAM_BENCH_DB = 'C:\path\to\vamlibrary.db'
cargo test --locked --manifest-path src-tauri/Cargo.toml benchmark_local_missing_dependencies -- --ignored --nocapture
```

基准以同一数据库快照比较旧算法和新算法，逐项校验缺失记录，并输出数据量、快照读取耗时和新算法 10 次匹配的中位数 / P95；它不代表完整页面加载耗时。

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
4. 根据需要进行去重、迁移、快捷入库、下载或分享。

### 全目录去重与平铺迁移

- 去重直接递归扫描所选目录中的所有普通文件，无需先入库。`.var` 按作者与资源名分组，保留最高数字版本；版本相同保留修改时间最新的一份。
- `meta.json` 中明确引用的旧版本保留并集中到资源根目录的 `依赖旧版本/`；`latest` 引用不固定旧版本。固定版本依赖只需保留一份副本。依赖无法读取时显示异常，暂时保留各版本；异常分组的同版本文件经 SHA-256 确认内容完全相同后，仍可回收多余副本并保留修改时间最新的一份。内容不同的文件保留并提示冲突。
- 清理相同副本时只检查文件清单和相关包的内容摘要，不重新解析其他包的场景与预设。删除某个版本的全部副本时仍执行完整引用检查。
- 其他文件仅在同名且 SHA-256 内容摘要相同时去重。清理文件进入回收站，历史单项由用户确认后手动清空；新批次默认关闭自动永久删除；归档旧版本不会进入回收站。
- 推荐先确认去重页的“回收 / 归档”清单，再选择迁移的“平铺到根目录”、真实目录及移动。迁移覆盖未索引包、文档、图片等全部普通文件；平铺保留 `依赖旧版本/` 目录。
- 预览检查磁盘目标和批次内同名冲突：VAR 不改名、不覆盖，冲突项留在原位置并提示先去重；其他同名文件添加序号，保留双方。
- 移动后仅清理源目录的空子目录；复制不清理目录。根目录、非空目录、链接、下载暂存目录及已知托管映射不会被清理。
- 执行使用后端保存的快照，并重新核对文件大小和修改时间；目录变化后需要重新扫描。迁移和旧版本归档均记录回滚信息，回滚不覆盖已有文件，并拒绝已变化的目标。

### 下载失败与文件保留

- 连接超时、临时网络错误以及 HTTP 408/429/500/502/503/504 最多尝试三次；401/403/404 直接提示登录、权限或地址问题。连接失败保留临时下载，续传前核对返回的字节范围。
- 下载完成后检查 ZIP 结构和各文件 CRC。仅 `meta.json` 格式异常时保留完整文件，显示持续警告并建立不含依赖元数据的基础索引，不再误删或反复下载；这不等于修复了资源作者的元数据，也不保证 VaM 能正常加载。
- 数据库入库失败仍保留文件，可以使用“重试入库”复用已保存的文件，不重新下载。已有资源文件不会被静默覆盖。

### 数据与文件安全说明

- 本地 SQLite 索引用于缓存扫描结果、依赖关系、标签、迁移记录和下载状态。
- 清空本地数据只会清除应用数据库，不会删除真实的 VAM 资源文件。
- 删除和去重清理会优先进入 VAM Library 回收站，新批次默认建议保留 30 天且不自动永久删除，历史单项不会自动删除。
- 资源迁移、快捷入库和下载操作会涉及真实文件的移动或复制，执行前请确认预览内容。

### 参与开发

- 前端使用 Vue 3 `<script setup>` 和 TypeScript。
- 所有 Tauri 命令统一在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中注册。
- 新增跨端能力时，需要同时关注 Rust 命令、前端 `invoke` 调用和 Tauri 权限配置。
- 大型 VAM 目录可能包含上百 GB 资源和大量 `.var` 文件，扫描、去重和迁移功能需要避免阻塞 UI。

### 开源许可

本项目采用 [MIT License](./LICENSE) 开源。Copyright (c) 2026 xyuqikzz。

> VAM Library 是非官方的第三方工具，与 Virt-A-Mate 官方无关。
