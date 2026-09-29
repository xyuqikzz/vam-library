using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using BepInEx;
using HarmonyLib;
using MVR.FileManagement;
using uFileBrowser;
using UnityEngine;
using Item = uFileBrowser.FileBrowser.FileAndDirInfo;

namespace VamLibrary.SceneBrowser
{
    [BepInPlugin("com.vamlibrary.scenebrowser", "VAM Library Scene Browser", "1.0.2")]
    public sealed class SceneBrowserPlugin : BaseUnityPlugin
    {
        private const string Author = "作者 A–Z";
        private const string Folders = "按文件夹浏览";
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
        private static string Root { get { return Path.Combine(Paths.GameRootPath, "AddonPackages"); } }

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
                Logger.LogInfo("Scene browser 1.0.2 ready: author sorting and folder cards in scene preview.");
            }
            catch (Exception e)
            {
                if (harmony != null) harmony.UnpatchSelf();
                Logger.LogError(e);
            }
        }

        private void OnDestroy()
        {
            if (harmony != null) harmony.UnpatchSelf();
            States.Clear();
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
                if (value == Author || value == Folders) continue;
                values.Add(value);
                labels.Add(popup.useDifferentDisplayValues ? popup.displayPopupValues[i] : value);
            }
            if (Active(browser)) { values.Add(Author); labels.Add(Author); values.Add(Folders); labels.Add(Folders); }
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
                if (Active(__instance) && (__0 == Author || __0 == Folders))
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
                    SyncSort.Invoke(__instance, null);
                    Menu(__instance);
                    return false; // Custom choices never enter VaM's persisted SortBy enum.
                }
                state.Mode = null;
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
                __0.Sort(delegate(Item a, Item b)
                {
                    var ap = a.FileEntry as VarFileEntry;
                    var bp = b.FileEntry as VarFileEntry;
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
