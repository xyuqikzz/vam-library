using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using BepInEx;
using HarmonyLib;
using MVR.FileManagement;
using uFileBrowser;
using UnityEngine;
using Item = uFileBrowser.FileBrowser.FileAndDirInfo;

namespace VamLibrary.SceneBrowser
{
    public sealed partial class SceneBrowserPlugin
    {
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
            state.FolderLinks = null;
        }

        private static List<Item> FolderItems(FileBrowser browser, BrowserState state)
        {
            if (state.FolderItems != null) return state.FolderItems;
            var links = state.FolderLinks ?? BrowserRules.ListFolders(Root, state.Folder);
            state.FolderLinks = links;
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
            var links = BrowserRules.ListFolders(Root, folder);
            var state = State(browser);
            ClearFolderItems(browser, state);
            state.Folder = folder;
            state.FolderLinks = links;
            syncSort(browser);
        }

        private static string FolderOf(VarPackage package)
        {
            string folder;
            if (!PackageFolders.TryGetValue(package.FullPath, out folder))
                PackageFolders[package.FullPath] = folder = BrowserRules.PackageFolder(Root, package.FullPath);
            return folder;
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
                if (state.FolderFilter && !state.Navigating)
                {
                    state.FolderFilter = false;
                    if (instance != null) instance.lastFolders.Value = false;
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
                if (!Active(__instance) || !State(__instance).FolderFilter || __0 == null ||
                    __0.fullPath == null || !__0.fullPath.StartsWith(FolderPrefix, StringComparison.Ordinal)) return true;
                try
                {
                    NavigateFolder(__instance, __0.fullPath.Substring(FolderPrefix.Length));
                }
                catch (Exception e)
                {
                    instance.Logger.LogError(e);
                    if (__instance.statusField != null) __instance.statusField.text = BrowserText.Get("folder-error");
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
                if (!Active(__instance) || !state.FolderFilter) return;
                var all = (List<Item>)Sorted.GetValue(__instance);
                if (all == null) return;
                // This scene preview has no visible dirContent sidebar. Folder cards
                // must participate in the same grid and pagination as scene thumbnails.
                var visible = new List<Item>(FolderItems(__instance, state));
                var counts = new Dictionary<string, int>(StringComparer.OrdinalIgnoreCase);
                foreach (var item in all)
                {
                    var entry = item.FileEntry as VarFileEntry;
                    string folder = entry == null ? null : FolderOf(entry.Package);
                    string child = BrowserRules.ChildFolder(state.Folder, folder);
                    if (child != null) { int count; counts.TryGetValue(child, out count); counts[child] = count + 1; }
                    bool keep = entry != null && string.Equals(folder, state.Folder, StringComparison.OrdinalIgnoreCase);
                    if (keep) visible.Add(item);
                    else if (item.button != null) hideButton(__instance, item.button);
                }
                for (int i = 0; i < state.FolderItems.Count; i++)
                {
                    var link = state.FolderLinks[i];
                    if (link.Navigation) continue;
                    int count; counts.TryGetValue(link.RelativePath, out count);
                    var button = state.FolderItems[i].button;
                    string label = link.Label + " · " + count;
                    if (button.text == label) continue;
                    button.text = label; button.textLowerInvariant = label.ToLowerInvariant();
                    if (button.label != null) button.label.text = label;
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
