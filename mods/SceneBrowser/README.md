# VaM 管理增强插件 1.2.0

供 VAM Library 安装的 BepInEx 5 插件。基于 VaM 1.22.0.13 / Unity CLR 2 构建，保留原生场景加载、安全提示、收藏、搜索和分页。

## 功能

- 普通加载与合并加载场景选择器都支持作者 A–Z、入库时间（新到旧）、资源包大小双向排序。作者内的场景名与文件夹名使用自然数字顺序（2 在 10 前），中文按码位排序。
- 排序存入 BepInEx 配置 `[Browser] LastSceneSort`，使用 `author`、`imported`、`size-desc`、`size-asc` 标识，不写进游戏偏好。选择原生排序后清除自定义排序记忆。
- 「按文件夹浏览」是独立开关，可与任意排序叠加，再选一次或使用原生快捷目录退出。是否启用存入 `[Browser] FolderFilter`，重新打开时回到 AddonPackages 根目录。文件夹导航卡片参与原生分页，显示该直接子目录及更深层的场景总数；搜索和收藏不隐藏返回入口。
- 目录链接可以进入，导航只列当前层，不递归遍历。VaM 的 `FileManager.Refresh` 使用 `Directory.GetFiles(..., "*.var", SearchOption.AllDirectories)`；运行时探针已用游戏自带 Mono 和临时目录 junction 验证同一发现方式保留链接路径。隐藏内部目录仍不显示。
- 软件导出的 `import-times.tsv` 使用 `packages.scan_time`，不是文件创建时间。游戏在入库时间排序开启时每秒检查文件变化；未变化不重置分页。移动、重复扫描保留首次入库时间，不同版本分开记录。
- 软件同步入库时间和切换语言时同时更新 `app-settings.tsv`。游戏每次打开选择器时读取中英文设置，按文件时间和大小跳过重复读取；无文件时使用中文。排序菜单保留原生显示机制，内部将两种语言的文字映射到稳定标识。

文件夹过滤缓存包路径到所在文件夹的关系，并使用开放实例委托调用 `HideButton` / `SyncSort`，避免热路径反射参数数组和重复完整路径解析。文件夹导航复用已完成的目录读取，计数未变化时不更新卡片文字。

## 场景启动协议 v2

点击软件中的「启动场景」会切换当前游戏场景，请先保存正在编辑的内容。支持桌面模式和 OpenVR；当前游戏目录已有进程时沿用其模式，不启动第二个进程。同一目录的多个 VaM 进程会拒绝联动。

请求、受理和结果文件位于 `BepInEx/plugins/VamLibrary.SceneBrowser/scene-launch`，无网络监听：

- `{id}.request`：版本头、独立编号、创建时间（UTC 毫秒）、场景路径；先写临时文件再原子改名。
- 软件等待时每 5 秒更新请求修改时间。游戏要求心跳不早于 15 秒前，创建时间不晚于未来 5 秒且不超过 30 分钟；重扫资源后再次核对心跳，避免软件退出后的迟到加载。
- `{id}.accepted`：校验场景和包状态后、调用 `Load` 前原子写入。
- `{id}.result`：版本头、编号、`loaded` 或 `error`，错误时附稳定错误码与可选原始详情。软件翻译错误码，诊断信息默认折叠。

| 阶段 | 软件提示 | 等待上限 |
| --- | --- | --- |
| 软件启动游戏，等待受理 | 正在打开游戏… | 15 分钟，进程退出立即失败 |
| 游戏已运行，等待受理 | 正在把场景发给游戏… | 2 分钟 |
| 游戏受理后 | 游戏已打开，正在加载场景… | 10 分钟；之后提示到游戏窗口查看，不报告加载失败 |

模组记录调用前场景名，并观察真实加载状态；同一场景重开不能在未开始加载时报告成功。60 秒没有进入加载或改变场景名，返回 `load_not_started`。原生安全确认与依赖提示继续保留。

完成、超时和软件端任务释放时清理本次协议文件。游戏启动后清理心跳失效的请求，以及超过 30 分钟的结果、受理和临时文件。软件关闭后不再更新心跳；已经交给游戏的加载不会被强制停止。v2 不兼容旧协议，软件要求安装与内置 DLL 字节相同的插件。

## 安装、兼容检查与归属

在「设置 → VaM 管理增强插件」安装、更新或卸载。只检查当前游戏目录的 VaM 进程；其他目录的游戏不会阻止操作。不修改游戏程序集、VAR、存档或偏好，不安装或升级 BepInEx。

`scene-browser.json` 保留 Assembly-CSharp、BepInEx 和 Harmony 三个文件的构建指纹。缺依赖禁止安装；完整匹配正常安装；指纹不同标为「未验证的游戏版本」，确认后才安装。插件启动时反射检查排序与场景联动 API，分别降级并写入 `plugin-status.tsv`；启动轮询出现运行时异常后停止联动并记录一次日志。设置页显示当前插件版本的上次启动结果，联动不可用时软件立即提示。

安装位置：`BepInEx/plugins/VamLibrary.SceneBrowser/VamLibrary.SceneBrowser.dll`。安装和更新同时写入 `VamLibrary.SceneBrowser.owner`，保存实际 DLL 的 SHA-256。只有当前内置 DLL、精确匹配标记的 DLL 或冻结的历史哈希名单允许更新和卸载。文件变更后不再认作自有插件。卸载删除 DLL 和归属标记，保留入库记录、语言配置和 BepInEx 配置。

历史哈希覆盖 1.0.0 至 1.0.9，包括两份经过验证的 1.0.8 构建。1.0.9 回归样本保留在 `tests/fixtures`，不打包到应用。新版本不再手动追加历史哈希。

原生 VAR 解压已移除：已有基准不足以支持可感知的场景加载收益，并给游戏读取小文件增加了检查。旧 `[Performance] NativeDecompression` 配置不再读取。SharpZipLib 仅作为编译引用解析游戏签名，插件 DLL 不含运行时引用。

## 构建与验证

使用已安装的 .NET SDK Roslyn，以 `/deterministic+ /noconfig /nostdlib+ /langversion:5` 引用游戏自带 CLR 2 程序集。构建只读取游戏依赖，输出到本仓库，不部署插件。`test-build.ps1` 进行两次真实编译并比较全部字节，不再依赖忽略元数据的指纹。DLL 与兼容清单需一起更新；应用通过 `include_bytes!` 内置插件，必须重新构建应用。

```powershell
./mods/SceneBrowser/build.ps1 -VamRoot '<VaM 根目录>'
./mods/SceneBrowser/test-build.ps1 -VamRoot '<VaM 根目录>'
./mods/SceneBrowser/test.ps1 -VamRoot '<VaM 根目录>'
./mods/SceneBrowser/test-runtime.ps1 -VamRoot '<VaM 根目录>'
npm run build
npm test
cargo test --locked --manifest-path src-tauri/Cargo.toml
$env:VAM_MOD_TEST_ROOT = '<VaM 根目录>'
cargo test --locked --manifest-path src-tauri/Cargo.toml installation_round_trip_with_local_game -- --ignored
```

本机安装往返测试只把真实依赖复制到独立临时目录，覆盖安装、重复安装、历史版本升级、归属标记、卸载和资源保留，不操作真实游戏安装。Mono 探针独立运行，覆盖类型加载、运行时自检、九个 Harmony 补丁、开放实例委托、Unicode 请求、大小排序、目录链接发现与导航条目，不注入运行中的游戏。

仍需真实游戏验收：超过三分钟的冷启动、软件中途退出后的失效请求、桌面 / VR 场景切换、原生确认提示、重开同一场景、排序恢复和分页、合并加载、文件夹与搜索 / 收藏 / 排序组合、两千以上场景的连续搜索，以及贴图较多场景的完整加载。独立 Mono 和自动化测试不能代替这些实际游戏路径。
