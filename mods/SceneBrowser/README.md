# VAM Library 场景浏览增强 1.0.2

1.0.2 修复场景预览器没有可见侧栏时文件夹模式显示空白的问题。文件夹、返回上级、返回根目录都作为原生卡片放入场景缩略图网格，参与分页，搜索和收藏筛选不会隐藏导航卡片。软件支持哈希匹配的 1.0.0 / 1.0.1 直接更新。

1.0.1 修复旧版在 VaM CLR 2 中因 `ConditionalWeakTable` 和系统 CLR 4 引用而无法加载的问题。现在使用游戏自带的 mscorlib / System / System.Core 编译。已安装 1.0.0 时，退出游戏并在软件中点击「更新模组」；只更新哈希匹配的已知旧版，不覆盖改动过的 DLL。

这是游戏内 BepInEx 模组，通过 VAM Library 的「设置 → 游戏模组 · 场景浏览增强」安装。目标为 VaM 1.22.0.13；本次构建使用的游戏、BepInEx 5 和 Harmony 二进制指纹保存在 `src-tauri/resources/mods/scene-browser.json`。安装器校验这些指纹，不会覆盖其他插件或安装 / 升级加载器。

## 使用

1. 退出 VaM，在 VAM Library 设置中选中游戏根目录。
2. 在「游戏模组 · 场景浏览增强」中点击「安装模组」。
3. 启动游戏，打开「选择要加载的场景」，在原来的排序菜单中选择：
   - **作者 A–Z**：按 VAR 元数据中的作者排序，忽略大小写；同作者按场景名排序。无作者的本地场景放在后面。
   - **按文件夹浏览**：场景缩略图网格中先显示 `[文件夹]` 卡片，再显示当前层 VAR 包里的场景。点击文件夹进入，点击 `.. / 返回上一级` 或 `返回 AddonPackages 根目录` 卡片返回。无需侧边目录栏，根目录没有直接存放 VAR 时仍然显示子文件夹入口。
4. 选择原有排序方式，或使用原生场景快捷目录，退出文件夹模式。

文件夹模式不递归混合子目录场景，不把 VAR 内部的 `Saves/scene` 当作物理目录。空文件夹和不含场景的文件夹仍可浏览，并保留返回卡片。目录链接与点号开头的内部目录不进入；未被 VaM 索引或禁用的 VAR 不会出现。搜索、隐藏、收藏、分页和点击加载继续由原生浏览器处理，筛选只作用于当前文件夹内的场景，导航卡片始终参与分页。计数包括文件夹卡片。新打开场景选择器时恢复原生排序，不把自定义值写入游戏偏好设置。仅扩展「选择要加载的场景」，不改动外观、保存等其他选择器。

安装位置：`BepInEx/plugins/VamLibrary.SceneBrowser/VamLibrary.SceneBrowser.dll`。卸载前退出游戏，再在同一软件入口点击「卸载模组」。安装和卸载不移动任何 VAR，不修改游戏程序集、存档或偏好。安装器仅识别当前版本及指定哈希的旧版 1.0.0 / 1.0.1，其他内容的同名 DLL 会拒绝覆盖和删除。

## 开发和验证

在仓库根目录用 PowerShell 7 执行，`<VaM 根目录>` 为装有兼容 BepInEx 5 的游戏目录：

```powershell
./mods/SceneBrowser/build.ps1 -VamRoot '<VaM 根目录>'
./mods/SceneBrowser/test.ps1 -VamRoot '<VaM 根目录>'
./mods/SceneBrowser/test-runtime.ps1 -VamRoot '<VaM 根目录>'
npm run build
cargo test --manifest-path src-tauri/Cargo.toml game_mods
$env:VAM_MOD_TEST_ROOT = '<VaM 根目录>'
cargo test --manifest-path src-tauri/Cargo.toml installation_round_trip_with_local_game -- --ignored
npm run tauri -- build --no-bundle
```

构建脚本仅从游戏目录读取程序集，输出本项目的 DLL 和兼容性清单，不部署到游戏。Rust 通过 `include_bytes!` 把模组内置到软件，不依赖运行时源码目录。DLL 与清单需一起保留；重建 DLL 后必须重建软件。游戏程序集和第三方加载器不会打包到软件。重新构建针对其他游戏版本的模组后，还必须核对 API 契约并完成游戏内验收，不能仅以编译成功判断兼容。

规则测试覆盖作者分组、大小写、无作者、同名稳定排序、根目录、中文嵌套目录、空目录返回、根层无 VAR 的文件夹入口及路径越界。二进制契约检查覆盖 Harmony 钩子及 CLR 2 引用。`test-runtime.ps1` 在独立进程内嵌入游戏自带的 Mono，复现旧版加载失败，并验证修正版类型加载、静态初始化、浏览器状态创建及全部 8 个 Harmony 补丁挂载 / 卸载，也使用实际 AddonPackages 目录构造原生导航网格条目；不注入运行中的游戏。旧 DLL 仅作为回归测试样本保留，不打包进软件。Rust 测试覆盖依赖缺失、未知 DLL 保护、卸载保护。显式运行的本机环境测试只读取真实依赖并复制到独立临时目录，验证完整安装、重复安装、旧版升级、哈希和卸载。

仍需游戏内验收：桌面与 VR 场景浏览器中的菜单布局、跨页作者分组、文件夹进入 / 返回、空目录、搜索 / 收藏组合筛选，以及切回全部场景后的点击加载。编译和单元测试不替代这些运行检查。
