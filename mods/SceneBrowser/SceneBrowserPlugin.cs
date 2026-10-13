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
    [BepInPlugin("com.vamlibrary.scenebrowser", "VaM 管理增强插件", "1.2.0")]
    public sealed partial class SceneBrowserPlugin : BaseUnityPlugin
    {
        private const string Author = "author";
        private const string Folders = "folders";
        private const string Imported = "imported";
        private const string SizeDescending = "size-desc";
        private const string SizeAscending = "size-asc";
        private static string MissingTimes { get { return BrowserText.Get("missing"); } }
        private static string InvalidTimes { get { return BrowserText.Get("invalid"); } }
        private static string rootPath;
        private static readonly Dictionary<string, string> PackageFolders = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        private static Action<FileBrowser, FileButton> hideButton;
        private static Action<FileBrowser> syncSort;
        private BepInEx.Configuration.ConfigEntry<string> lastSort;
        private BepInEx.Configuration.ConfigEntry<bool> lastFolders;
        private bool browserAvailable, launchAvailable;
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
        private readonly ImportTimeIndex importTimes = new ImportTimeIndex();
        private float nextImportCheck;
        private float nextLaunchCheck;
        private static SuperController readyController;
        private static float sceneReadyAt;
        private SceneLaunchRequest pendingLaunch;
        private string pendingLaunchFile;
        private LaunchProgress launchProgress;
        private static string LaunchDirectory { get { return Path.Combine(Paths.PluginPath, "VamLibrary.SceneBrowser/scene-launch"); } }
        private static string Root { get { return rootPath; } }
        private static string ImportTimesPath { get { return Path.Combine(Paths.PluginPath, "VamLibrary.SceneBrowser/import-times.tsv"); } }

        private sealed class BrowserState
        {
            public BrowserState(FileBrowser browser) { Browser = new WeakReference(browser); }
            public readonly WeakReference Browser;
            public bool Scene;
            public string Mode;
            public string Folder = "";
            public bool Navigating;
            public bool FolderFilter;
            public bool Restore;
            public List<BrowserRules.FolderLink> FolderLinks;
            public List<Item> FolderItems;
        }

        private void Awake()
        {
            instance = this;
            rootPath = Path.GetFullPath(Path.Combine(Paths.GameRootPath, "AddonPackages")).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            lastSort = Config.Bind("Browser", "LastSceneSort", "", "Last scene sort (independent of game preferences).");
            lastFolders = Config.Bind("Browser", "FolderFilter", false, "Restore folder browsing at AddonPackages root.");
            harmony = new Harmony("com.vamlibrary.scenebrowser");
            try
            {
                CheckBrowserContract();
                hideButton = (Action<FileBrowser, FileButton>)Delegate.CreateDelegate(typeof(Action<FileBrowser, FileButton>), HideButton);
                syncSort = (Action<FileBrowser>)Delegate.CreateDelegate(typeof(Action<FileBrowser>), SyncSort);
                foreach (Type patch in new[] { typeof(TitlePatch), typeof(ShowPatch), typeof(SelectSortPatch), typeof(SortPatch), typeof(NavigatePatch), typeof(FolderClickPatch), typeof(HidePatch), typeof(DisplayPatch) })
                    harmony.CreateClassProcessor(patch).Patch();
                browserAvailable = true;
            }
            catch (Exception e) { harmony.UnpatchSelf(); Logger.LogError(e); }
            try
            {
                CheckLaunchContract();
                harmony.CreateClassProcessor(typeof(SceneLaunchReady)).Patch();
                launchAvailable = true;
            }
            catch (Exception e) { Logger.LogError(e); }
            WriteStatus();
            Logger.LogInfo("VaM 管理增强插件 1.2.0: browser=" + browserAvailable + ", launch=" + launchAvailable);
        }

        private void WriteStatus()
        {
            try
            {
                string directory = Path.Combine(Paths.PluginPath, "VamLibrary.SceneBrowser");
                Directory.CreateDirectory(directory);
                string target = Path.Combine(directory, "plugin-status.tsv");
                string temp = target + ".tmp";
                File.WriteAllText(temp, "# VAM Library plugin status v1\nversion\t1.2.0\nbrowser\t" + (browserAvailable ? "ok" : "unavailable") + "\nlaunch\t" + (launchAvailable ? "ok" : "unavailable") + "\n", new UTF8Encoding(false));
                if (File.Exists(target)) File.Replace(temp, target, null); else File.Move(temp, target);
            }
            catch (Exception e) { Logger.LogWarning(e); }
        }

        private void OnDestroy()
        {
            if (harmony != null) harmony.UnpatchSelf();
            States.Clear();
            PackageFolders.Clear();
            readyController = null;
        }

        private void Update()
        {
            if (launchAvailable)
            {
                try { CheckSceneLaunch(); }
                catch (Exception e)
                {
                    launchAvailable = false;
                    Logger.LogError(e);
                    if (pendingLaunch != null)
                    {
                        try { FinishLaunch("error", "internal", e.Message); }
                        catch (Exception completionError) { Logger.LogWarning(completionError); }
                    }
                    WriteStatus();
                }
            }
            if (!browserAvailable) return;
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
                foreach (var browser in browsers) syncSort(browser);
            foreach (var browser in browsers) ImportStatus(browser);
        }

        private static void ImportStatus(FileBrowser browser)
        {
            if (browser.statusField == null) return;
            string warning = instance.importTimes.Error != null ? InvalidTimes : instance.importTimes.Count == 0 ? MissingTimes : null;
            if (warning != null) browser.statusField.text = warning;
            else if (BrowserText.IsImportStatus(browser.statusField.text)) browser.statusField.text = "";
        }

        private static void ClearImportStatus(FileBrowser browser)
        {
            if (browser.statusField != null && (BrowserText.IsImportStatus(browser.statusField.text)))
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

    }
}
