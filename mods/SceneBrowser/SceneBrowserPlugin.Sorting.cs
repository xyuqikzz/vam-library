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
        private static void Menu(FileBrowser browser)
        {
            UIPopup popup = browser.sortByPopup;
            if (popup == null) return;
            var values = new List<string>();
            var labels = new List<string>();
            for (int i = 0; i < popup.numPopupValues; i++)
            {
                string value = popup.popupValues[i];
                if (BrowserText.Mode(value) != null) continue;
                values.Add(value);
                labels.Add(popup.useDifferentDisplayValues ? popup.displayPopupValues[i] : value);
            }
            if (Active(browser))
            {
                values.Add(BrowserText.Get(Author)); labels.Add(BrowserText.Get(Author));
                values.Add(BrowserText.Get(Folders)); labels.Add(BrowserText.Get(Folders));
                values.Add(BrowserText.Get(Imported)); labels.Add(BrowserText.Get(Imported));
                values.Add(BrowserText.Get(SizeDescending)); labels.Add(BrowserText.Get(SizeDescending));
                values.Add(BrowserText.Get(SizeAscending)); labels.Add(BrowserText.Get(SizeAscending));
            }
            if (!values.SequenceEqual(popup.popupValues))
            {
                popup.numPopupValues = values.Count;
                for (int i = 0; i < values.Count; i++)
                {
                    popup.setPopupValue(i, values[i]);
                    popup.setDisplayPopupValue(i, labels[i]);
                }
            }
            popup.currentValueNoCallback = State(browser).Mode == null ? browser.sortBy.ToString() : BrowserText.Get(State(browser).Mode);
        }

        [HarmonyPatch(typeof(FileBrowser), "SetTitle")]
        private static class TitlePatch
        {
            private static void Postfix(FileBrowser __instance, string __0)
            {
                var state = State(__instance);
                ClearFolderItems(__instance, state);
                BrowserText.Refresh(Path.Combine(Paths.PluginPath, "VamLibrary.SceneBrowser/app-settings.tsv"));
                state.Scene = __0 == "Select Scene To Load" || __0 == "Select Scene For Merge";
                state.Restore = state.Scene;
                state.FolderFilter = false;
                state.Mode = null;
                state.Folder = "";
                ClearImportStatus(__instance);
                Menu(__instance);
            }
        }

        [HarmonyPatch(typeof(FileBrowser), "ShowInternal")]
        private static class ShowPatch
        {
            private static void Postfix(FileBrowser __instance)
            {
                var state = State(__instance);
                if (Active(__instance) && state.Restore)
                {
                    state.Restore = false;
                    string mode = instance == null ? null : BrowserText.Mode(instance.lastSort.Value);
                    if (mode != Folders) state.Mode = mode;
                    if (instance != null && instance.lastFolders.Value) ToggleFolders(__instance, true);
                    if (state.Mode != null || state.FolderFilter) syncSort(__instance);
                    if (state.Mode == Imported) ImportStatus(__instance);
                }
                Menu(__instance);
            }
        }

        private static void ApplyMode(FileBrowser browser, string mode)
        {
            var state = State(browser);
            if (mode == Folders) ToggleFolders(browser, !state.FolderFilter);
            else { state.Mode = mode; if (instance != null) instance.lastSort.Value = mode; }
            if (state.Mode == Imported) instance.importTimes.Refresh(ImportTimesPath, true);
            syncSort(browser);
            Menu(browser);
            if (state.Mode == Imported) ImportStatus(browser); else ClearImportStatus(browser);
        }

        private static void ToggleFolders(FileBrowser browser, bool enabled)
        {
            var state = State(browser);
            ClearFolderItems(browser, state);
            state.FolderFilter = enabled; state.Folder = "";
            if (instance != null) instance.lastFolders.Value = enabled;
            // Both entry and exit refresh the complete native scene collection.
            state.Navigating = true;
            try { browser.GotoDirectory("Saves/scene", null, true, true); }
            finally { state.Navigating = false; }
        }

        [HarmonyPatch(typeof(FileBrowser), "SetSortBy")]
        private static class SelectSortPatch
        {
            private static bool Prefix(FileBrowser __instance, string __0)
            {
                if (!Active(__instance)) return true;
                var state = State(__instance);
                string mode = BrowserText.Mode(__0);
                if (Active(__instance) && mode != null)
                {
                    ApplyMode(__instance, mode);
                    return false; // Custom choices never enter VaM's persisted SortBy enum.
                }
                state.Mode = null;
                if (Active(__instance) && instance != null) instance.lastSort.Value = "";
                ClearImportStatus(__instance);
                // Native setter skips refresh when the old enum equals the selected enum.
                syncSort(__instance);
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
                bool size = State(__instance).Mode == SizeDescending || State(__instance).Mode == SizeAscending;
                bool descending = State(__instance).Mode == SizeDescending;
                if (imported) instance.importTimes.Refresh(ImportTimesPath, false);
                __0.Sort(delegate(Item a, Item b)
                {
                    var ap = a.FileEntry as VarFileEntry;
                    var bp = b.FileEntry as VarFileEntry;
                    if (size)
                        return BrowserRules.CompareSize(
                            ap == null ? (long?)null : ap.Package.Size,
                            bp == null ? (long?)null : bp.Package.Size, descending,
                            ap == null ? null : ap.Package.Creator, a.Name, a.FullName,
                            bp == null ? null : bp.Package.Creator, b.Name, b.FullName);
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

    }
}
