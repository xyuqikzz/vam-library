using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using BepInEx;
using BepInEx.Configuration;
using HarmonyLib;
using MVR.FileManagement;
using uFileBrowser;
using UnityEngine;
using Item = uFileBrowser.FileBrowser.FileAndDirInfo;

namespace VamLibrary.SceneBrowser
{
    [BepInPlugin("com.vamlibrary.scenebrowser", "vam管理增强插件", "1.0.8")]
    public sealed class SceneBrowserPlugin : BaseUnityPlugin
    {
        private const string Author = "作者 A–Z";
        private const string Folders = "按文件夹浏览";
        private const string Imported = "入库时间";
        private const string MissingTimes = "暂无入库记录，在软件中入库或刷新资源后自动更新。";
        private const string InvalidTimes = "读取入库时间失败，请在软件中刷新资源。";
        private const string FolderPrefix = "vamlibrary-folder:";
        // VaM 1.x runs Unity's CLR 2 profile, which has no ConditionalWeakTable.
        private static readonly List<BrowserState> States = new List<BrowserState>();
        private static readonly FieldInfo Sorted = AccessTools.Field(typeof(FileBrowser), "sortedFilesAndDirs");
        private static readonly FieldInfo Displayed = AccessTools.Field(typeof(FileBrowser), "displayedFileButtons");
        private static readonly PropertyInfo DirectoryEntryProperty = AccessTools.Property(typeof(Item), "DirectoryEntry");
        private static readonly MethodInfo SyncSort = AccessTools.Method(typeof(FileBrowser), "SyncSort");
        private static readonly MethodInfo CreateFile = AccessTools.Method(typeof(FileBrowser), "CreateFileButton");
        private static readonly MethodInfo HideButton = AccessTools.Method(typeof(FileBrowser), "HideButton");
        private static SceneBrowserPlugin instance;
        private Harmony harmony;
        private ConfigEntry<bool> nativeDecompression;
        private string decompressionWarning;
        private readonly ImportTimeIndex importTimes = new ImportTimeIndex();
        private float nextImportCheck;
        private float nextLaunchCheck;
        private static SuperController readyController;
        private static float sceneReadyAt;
        private SceneLaunchRequest pendingLaunch;
        private string pendingLaunchFile;
        private string pendingLaunchError;
        private static string LaunchDirectory { get { return Path.Combine(Paths.PluginPath, "VamLibrary.SceneBrowser/scene-launch"); } }
        private static string Root { get { return Path.Combine(Paths.GameRootPath, "AddonPackages"); } }
        private static string ImportTimesPath { get { return Path.Combine(Paths.PluginPath, "VamLibrary.SceneBrowser/import-times.tsv"); } }

        private sealed class BrowserState
        {
            public BrowserState(FileBrowser browser) { Browser = new WeakReference(browser); }
            public readonly WeakReference Browser;
            public bool Scene;
            public string Mode;
            public string Folder = "";
            public bool Navigating;
            public List<Item> FolderItems;
        }

        private void Awake()
        {
            instance = this;
            try
            {
                if (Sorted == null || Displayed == null || DirectoryEntryProperty == null || SyncSort == null || CreateFile == null || HideButton == null)
                    throw new MissingMemberException("Unsupported VaM scene browser API");
                harmony = new Harmony("com.vamlibrary.scenebrowser");
                harmony.PatchAll(typeof(SceneBrowserPlugin).Assembly);
                // Performance hooks are isolated: failure keeps browser/launch features working.
                try
                {
                    nativeDecompression = Config.Bind("Performance", "NativeDecompression", true, "使用游戏自带 zlib 解压 VAR 资源，异常时回退原版。");
                    nativeDecompression.SettingChanged += OnPerformanceSettingChanged;
                    ApplyDecompression();
                }
                catch (Exception error) { NativeDecompression.Disable(); Logger.LogWarning("原生解压未启用: " + error.Message); }
                Logger.LogInfo("vam管理增强插件 1.0.8 ready: sorting, scene launch and native VAR decompression.");
            }
            catch (Exception e)
            {
                if (harmony != null) harmony.UnpatchSelf();
                Logger.LogError(e);
            }
        }

        private void OnDestroy()
        {
            if (nativeDecompression != null) nativeDecompression.SettingChanged -= OnPerformanceSettingChanged;
            NativeDecompression.Disable();
            if (harmony != null) harmony.UnpatchSelf();
            States.Clear();
            readyController = null;
        }

        private void Update()
        {
            ReportDecompressionWarning();
            CheckSceneLaunch();
            if (Time.unscaledTime < nextImportCheck) return;
            nextImportCheck = Time.unscaledTime + 1f;
            var browsers = new List<FileBrowser>();
            foreach (var state in States)
            {
                var browser = state.Browser.Target as FileBrowser;
                if (browser != null && state.Scene && state.Mode == Imported && !browser.IsHidden()) browsers.Add(browser);
            }
            if (browsers.Count == 0) return;
            if (importTimes.Refresh(ImportTimesPath, false))
                foreach (var browser in browsers) SyncSort.Invoke(browser, null);
            foreach (var browser in browsers) ImportStatus(browser);
        }

        private void OnPerformanceSettingChanged(object sender, EventArgs args) { ApplyDecompression(); }

        private void ApplyDecompression()
        {
            if (nativeDecompression.Value) NativeDecompression.Enable();
            else NativeDecompression.Disable();
            ReportDecompressionWarning();
        }

        private void ReportDecompressionWarning()
        {
            string error = NativeDecompression.Failure;
            if (error != null && error != decompressionWarning) Logger.LogWarning("原生解压已回退到原版: " + error);
            decompressionWarning = error;
        }

        // Start initializes the UI and starts VaM's delayed default-scene load.
        // Wait for both this lifecycle boundary and any native load to finish.
        [HarmonyPatch(typeof(SuperController), "Start")]
        private static class SceneLaunchReady
        {
            private static void Postfix(SuperController __instance)
            {
                readyController = __instance;
                sceneReadyAt = Time.unscaledTime + 1f;
            }
        }

        private void FinishLaunch(string state, string message)
        {
            var request = pendingLaunch;
            string path = pendingLaunchFile;
            SceneLaunchRequest.Complete(path, request.Id, state, message);
            pendingLaunch = null; pendingLaunchFile = null; pendingLaunchError = null;
        }

        private void CheckSceneLaunch()
        {
            if (Time.unscaledTime < nextLaunchCheck) return;
            nextLaunchCheck = Time.unscaledTime + .5f;
            try
            {
                if (pendingLaunch != null)
                {
                    if (!File.Exists(pendingLaunchFile)) { pendingLaunch = null; pendingLaunchFile = null; pendingLaunchError = null; return; }
                    if (pendingLaunchError != null) { FinishLaunch("error", pendingLaunchError); return; }
                    if (readyController == null || readyController.isLoading) return;
                    bool loaded = String.Equals(readyController.LoadedSceneName, pendingLaunch.Scene, StringComparison.OrdinalIgnoreCase);
                    FinishLaunch(loaded ? "loaded" : "error", loaded ? "" : "游戏未能加载所选场景，请检查游戏错误日志及依赖。");
                    return;
                }
                if (readyController == null || Time.unscaledTime < sceneReadyAt || readyController.isLoading || !Directory.Exists(LaunchDirectory)) return;
                foreach (string path in Directory.GetFiles(LaunchDirectory, "*.request"))
                {
                    string id = Path.GetFileNameWithoutExtension(path);
                    if (!SceneLaunchRequest.ValidId(id)) continue;
                    try
                    {
                        if (new FileInfo(path).Length > 8192) throw new InvalidDataException("场景启动请求过大。");
                        pendingLaunch = SceneLaunchRequest.Parse(File.ReadAllText(path, Encoding.UTF8), id, SceneLaunchRequest.Now);
                        pendingLaunchFile = path;
                        if (!FileManager.FileExists(pendingLaunch.Scene, false, false)) readyController.RescanPackages();
                        if (!FileManager.FileExists(pendingLaunch.Scene, false, false))
                        { FinishLaunch("error", "场景在游戏中不可用，请检查资源包是否禁用、按需映射是否启用及依赖是否齐全。"); return; }
                        int colon = pendingLaunch.Scene.IndexOf(":/");
                        if (colon >= 0)
                        {
                            var package = FileManager.GetPackage(pendingLaunch.Scene.Substring(0, colon));
                            if (package == null || !package.Enabled)
                            { FinishLaunch("error", "资源包已在游戏中禁用，请先启用该包及依赖再启动场景。"); return; }
                        }
                        // Explicit launch in the app is the user's load action.
                        // Preserve native plugin security and dependency prompts.
                        readyController.Load(pendingLaunch.Scene);
                        return;
                    }
                    catch (Exception e)
                    {
                        Logger.LogError(e);
                        if (pendingLaunch != null)
                        {
                            pendingLaunchError = e.Message;
                            FinishLaunch("error", e.Message);
                        }
                        else SceneLaunchRequest.Complete(path, id, "error", e.Message);
                    }
                }
            }
            catch (Exception e) { Logger.LogError(e); }
        }

        private static void ImportStatus(FileBrowser browser)
        {
            if (browser.statusField == null) return;
            string warning = instance.importTimes.Error != null ? InvalidTimes : instance.importTimes.Count == 0 ? MissingTimes : null;
            if (warning != null) browser.statusField.text = warning;
            else if (browser.statusField.text == MissingTimes || browser.statusField.text == InvalidTimes) browser.statusField.text = "";
        }

        private static void ClearImportStatus(FileBrowser browser)
        {
            if (browser.statusField != null && (browser.statusField.text == MissingTimes || browser.statusField.text == InvalidTimes))
                browser.statusField.text = "";
        }

        private static BrowserState State(FileBrowser browser)
        {
            for (int i = States.Count - 1; i >= 0; i--)
            {
                object target = States[i].Browser.Target;
                if (ReferenceEquals(target, browser)) return States[i];
                if (target == null) States.RemoveAt(i);
            }
            var state = new BrowserState(browser);
            States.Add(state);
            return state;
        }
        private static bool Active(FileBrowser browser) { return State(browser).Scene && !browser.selectDirectory; }

        private static void ClearFolderItems(FileBrowser browser, BrowserState state)
        {
            if (state.FolderItems == null) return;
            var displayed = (HashSet<FileButton>)Displayed.GetValue(browser);
            foreach (var item in state.FolderItems)
            {
                if (item.button == null) continue;
                if (displayed != null) displayed.Remove(item.button);
                item.button.gameObject.SetActive(false);
                UnityEngine.Object.Destroy(item.button.gameObject);
            }
            state.FolderItems = null;
        }

        private static List<Item> FolderItems(FileBrowser browser, BrowserState state)
        {
            if (state.FolderItems != null) return state.FolderItems;
            var links = BrowserRules.ListFolders(Root, state.Folder);
            state.FolderItems = new List<Item>();
            try
            {
                foreach (var link in links)
                {
                    string full = BrowserRules.ResolveFolder(Root, link.RelativePath);
                    var item = CreateNavigationItem(full);
                    item.button = (FileButton)CreateFile.Invoke(browser, new object[] {
                        link.Label, full, true, false, false, false, false, false, false });
                    state.FolderItems.Add(item);
                    // Scene browsers strip filename extensions; restore navigation
                    // labels so '.. / 返回上一级' and dotted folder names stay intact.
                    item.button.text = link.Label;
                    item.button.textLowerInvariant = link.Label.ToLowerInvariant();
                    if (item.button.label != null) item.button.label.text = link.Label;
                    item.button.fullPath = FolderPrefix + link.RelativePath;
                    item.button.gameObject.SetActive(false);
                    item.button.transform.SetParent(browser.fileContent, false);
                }
                return state.FolderItems;
            }
            catch { ClearFolderItems(browser, state); throw; }
        }

        private static Item CreateNavigationItem(string full)
        {
            var item = new Item(new SystemDirectoryEntry(full), Path.GetDirectoryName(full));
            // These are navigation controls, not searchable/favoritable resources.
            // Native SyncDisplayed permits entries with neither a FileEntry nor
            // DirectoryEntry, so Back remains available during search and favorites.
            DirectoryEntryProperty.SetValue(item, null, null);
            return item;
        }

        private static void NavigateFolder(FileBrowser browser, string folder)
        {
            BrowserRules.ResolveFolder(Root, folder);
            // Validate before discarding the current navigation controls.
            BrowserRules.ListFolders(Root, folder);
            var state = State(browser);
            ClearFolderItems(browser, state);
            state.Folder = folder;
            SyncSort.Invoke(browser, null);
        }

        private static void Menu(FileBrowser browser)
        {
            UIPopup popup = browser.sortByPopup;
            if (popup == null) return;
            var values = new List<string>();
            var labels = new List<string>();
            for (int i = 0; i < popup.numPopupValues; i++)
            {
                string value = popup.popupValues[i];
                if (value == Author || value == Folders || value == Imported) continue;
                values.Add(value);
                labels.Add(popup.useDifferentDisplayValues ? popup.displayPopupValues[i] : value);
            }
            if (Active(browser)) { values.Add(Author); labels.Add(Author); values.Add(Folders); labels.Add(Folders); values.Add(Imported); labels.Add(Imported); }
            if (!values.SequenceEqual(popup.popupValues))
            {
                popup.numPopupValues = values.Count;
                for (int i = 0; i < values.Count; i++)
                {
                    popup.setPopupValue(i, values[i]);
                    popup.setDisplayPopupValue(i, labels[i]);
                }
            }
            popup.currentValueNoCallback = State(browser).Mode ?? browser.sortBy.ToString();
        }

        [HarmonyPatch(typeof(FileBrowser), "SetTitle")]
        private static class TitlePatch
        {
            private static void Postfix(FileBrowser __instance, string __0)
            {
                var state = State(__instance);
                ClearFolderItems(__instance, state);
                state.Scene = __0 == "Select Scene To Load";
                state.Mode = null;
                state.Folder = "";
                ClearImportStatus(__instance);
                Menu(__instance);
            }
        }

        [HarmonyPatch(typeof(FileBrowser), "ShowInternal")]
        private static class ShowPatch
        {
            private static void Postfix(FileBrowser __instance) { Menu(__instance); }
        }

        [HarmonyPatch(typeof(FileBrowser), "SetSortBy")]
        private static class SelectSortPatch
        {
            private static bool Prefix(FileBrowser __instance, string __0)
            {
                var state = State(__instance);
                bool wasFolders = state.Mode == Folders;
                if (Active(__instance) && (__0 == Author || __0 == Folders || __0 == Imported))
                {
                    state.Mode = __0;
                    if (__0 == Folders)
                    {
                        ClearFolderItems(__instance, state);
                        state.Folder = "";
                        state.Navigating = true;
                        try { __instance.GotoDirectory("Saves/scene", null, true, true); }
                        finally { state.Navigating = false; }
                    }
                    else if (wasFolders) ClearFolderItems(__instance, state);
                    if (__0 == Imported) instance.importTimes.Refresh(ImportTimesPath, true);
                    SyncSort.Invoke(__instance, null);
                    Menu(__instance);
                    if (__0 == Imported) ImportStatus(__instance);
                    else ClearImportStatus(__instance);
                    return false; // Custom choices never enter VaM's persisted SortBy enum.
                }
                state.Mode = null;
                ClearImportStatus(__instance);
                if (wasFolders) ClearFolderItems(__instance, state);
                // Native setter skips refresh when the old enum equals the selected enum.
                SyncSort.Invoke(__instance, null);
                return true;
            }
        }

        [HarmonyPatch(typeof(FileBrowser), "SortFilesAndDirs")]
        private static class SortPatch
        {
            private static void Postfix(FileBrowser __instance, List<Item> __0)
            {
                if (!Active(__instance) || State(__instance).Mode == null) return;
                bool imported = State(__instance).Mode == Imported;
                if (imported) instance.importTimes.Refresh(ImportTimesPath, false);
                __0.Sort(delegate(Item a, Item b)
                {
                    var ap = a.FileEntry as VarFileEntry;
                    var bp = b.FileEntry as VarFileEntry;
                    if (imported)
                        return BrowserRules.CompareImported(
                            instance.importTimes.Find(ap == null ? null : ap.Package.Uid),
                            instance.importTimes.Find(bp == null ? null : bp.Package.Uid),
                            ap == null ? null : ap.Package.Creator, a.Name, a.FullName,
                            bp == null ? null : bp.Package.Creator, b.Name, b.FullName);
                    return BrowserRules.Compare(ap == null ? null : ap.Package.Creator, a.Name, a.FullName,
                        bp == null ? null : bp.Package.Creator, b.Name, b.FullName);
                });
            }
        }

        [HarmonyPatch(typeof(FileBrowser), "GotoDirectory")]
        private static class NavigatePatch
        {
            private static bool Prefix(FileBrowser __instance, string __0)
            {
                var state = State(__instance);
                if (Active(__instance) && __0 != null && __0.StartsWith(FolderPrefix, StringComparison.Ordinal))
                {
                    string folder = __0.Substring(FolderPrefix.Length);
                    try
                    {
                        NavigateFolder(__instance, folder);
                    }
                    catch (Exception e) { instance.Logger.LogError(e); }
                    return false;
                }
                if (state.Mode == Folders && !state.Navigating)
                {
                    state.Mode = null;
                    ClearFolderItems(__instance, state);
                    Menu(__instance);
                }
                return true;
            }
        }

        [HarmonyPatch(typeof(FileBrowser), "OnFileClick")]
        private static class FolderClickPatch
        {
            private static bool Prefix(FileBrowser __instance, FileButton __0)
            {
                if (!Active(__instance) || State(__instance).Mode != Folders || __0 == null ||
                    __0.fullPath == null || !__0.fullPath.StartsWith(FolderPrefix, StringComparison.Ordinal)) return true;
                try
                {
                    NavigateFolder(__instance, __0.fullPath.Substring(FolderPrefix.Length));
                }
                catch (Exception e)
                {
                    instance.Logger.LogError(e);
                    if (__instance.statusField != null) __instance.statusField.text = "读取 AddonPackages 文件夹失败: " + e.Message;
                }
                return false;
            }
        }

        [HarmonyPatch(typeof(FileBrowser), "Hide")]
        private static class HidePatch
        {
            private static void Postfix(FileBrowser __instance) { ClearFolderItems(__instance, State(__instance)); }
        }

        [HarmonyPatch(typeof(FileBrowser), "SyncDisplayed")]
        private static class DisplayPatch
        {
            private static void Prefix(FileBrowser __instance, out List<Item> __state)
            {
                __state = null;
                var state = State(__instance);
                if (!Active(__instance) || state.Mode != Folders) return;
                var all = (List<Item>)Sorted.GetValue(__instance);
                if (all == null) return;
                // This scene preview has no visible dirContent sidebar. Folder cards
                // must participate in the same grid and pagination as scene thumbnails.
                var visible = new List<Item>(FolderItems(__instance, state));
                foreach (var item in all)
                {
                    var entry = item.FileEntry as VarFileEntry;
                    bool keep = entry != null && string.Equals(BrowserRules.PackageFolder(Root, entry.Package.FullPath), state.Folder, StringComparison.OrdinalIgnoreCase);
                    if (keep) visible.Add(item);
                    else if (item.button != null) HideButton.Invoke(__instance, new object[] { item.button });
                }
                // Filter before native pagination/search/favorites; never mutate cachedFiles.
                __state = all;
                Sorted.SetValue(__instance, visible);
            }

            private static void Finalizer(FileBrowser __instance, List<Item> __state)
            {
                if (__state != null) Sorted.SetValue(__instance, __state);
            }
        }
    }
}
