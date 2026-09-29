# 自动打包与发布

工作流：**Windows build**，使用 Windows x64 构建环境。

- 推送 `main`：运行前端检查、Rust 测试，构建并上传安装包。
- GitHub Actions 中选择 **Windows build → Run workflow**：手动打包指定分支。
- 推送 `X.Y.Z` 或 `vX.Y.Z` 标签：同样构建，成功后自动创建 GitHub Release 并上传文件。标签必须匹配 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 中一致的版本号。普通分支构建不会创建 Release。

## 下载

打开仓库 **Actions → Windows build → 成功的运行 → Artifacts**，下载 `VAM-Library-windows-x64-版本-运行编号`。构建产物保留 30 天，包含：

- `*-setup.exe`：NSIS 安装包。
- `*.msi`：MSI 安装包。
- `*-portable.zip`：便携版，可解压运行 `vam-library.exe`。
- `SHA256SUMS.txt`：上述三个文件的 SHA-256 校验值。

版本发布后的相同文件也可在 **Releases** 页面下载。安装包未配置代码签名，Windows 可能提示未知发布者；安装包包含 WebView2 引导程序，未安装 WebView2 时需要网络下载运行环境。

## 维护

依赖安装使用 `npm ci`，Rust 使用 `--locked`；修改依赖后需同步提交对应锁文件。GitHub 使用内置 `GITHUB_TOKEN`，只有标签触发的发布任务获得仓库内容写权限，无需添加个人令牌。

场景浏览模组 DLL 已在仓库中，直接内置到应用。GitHub 不包含游戏程序集，不重新编译该 DLL，也不运行需要本地游戏的 ignored 集成测试。更新模组源码后，应先在本地按 `mods/SceneBrowser/README.md` 构建和验证，并一起提交新 DLL 与兼容性清单。

发布新版本时先更新并提交三个应用版本号，再创建和推送对应的 `X.Y.Z` 或 `vX.Y.Z` 附注标签。建议用 `git tag -a X.Y.Z -F 中文说明文件.md` 写入功能说明，发布页会读取标签备注。工作流不会替你创建标签或改变版本号。模组版本与应用版本独立，例如模组 1.0.2 可以内置在应用 0.1.0 中。
