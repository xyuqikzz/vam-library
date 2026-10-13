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
        // Start is VaM's lifecycle boundary; native default loading still has priority.
        [HarmonyPatch(typeof(SuperController), "Start")]
        private static class SceneLaunchReady
        {
            private static void Postfix(SuperController __instance)
            {
                readyController = __instance;
                sceneReadyAt = Time.unscaledTime + 1f;
                try { SceneLaunchRequest.Cleanup(LaunchDirectory, SceneLaunchRequest.Now); }
                catch (Exception e) { if (instance != null) instance.Logger.LogWarning(e); }
            }
        }

        private void FinishLaunch(string state, string code, string detail = null)
        {
            SceneLaunchRequest.Complete(pendingLaunchFile, pendingLaunch.Id, state, code, detail);
            pendingLaunch = null; pendingLaunchFile = null; launchProgress = null;
        }

        [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
        private void CheckSceneLaunch()
        {
            if (Time.unscaledTime < nextLaunchCheck) return;
            nextLaunchCheck = Time.unscaledTime + .5f;
            if (pendingLaunch != null)
            {
                if (!File.Exists(pendingLaunchFile)) { pendingLaunch = null; pendingLaunchFile = null; launchProgress = null; return; }
                if (readyController == null) return;
                string result = launchProgress.Observe(readyController.isLoading, readyController.LoadedSceneName, Time.unscaledTime);
                if (result != null) FinishLaunch(result == "loaded" ? "loaded" : "error", result == "loaded" ? "" : result);
                return;
            }
            if (readyController == null || Time.unscaledTime < sceneReadyAt || readyController.isLoading || !Directory.Exists(LaunchDirectory)) return;
            foreach (string path in Directory.GetFiles(LaunchDirectory, "*.request"))
            {
                string id = Path.GetFileNameWithoutExtension(path);
                if (!SceneLaunchRequest.ValidId(id)) continue;
                try
                {
                    var info = new FileInfo(path);
                    if (info.Length > 8192 || (info.Attributes & FileAttributes.ReparsePoint) != 0) throw new InvalidDataException("invalid_request");
                    pendingLaunch = SceneLaunchRequest.Parse(File.ReadAllText(path, Encoding.UTF8), id, SceneLaunchRequest.Now, SceneLaunchRequest.Milliseconds(info.LastWriteTimeUtc));
                    pendingLaunchFile = path;
                    if (!FileManager.FileExists(pendingLaunch.Scene, false, false)) readyController.RescanPackages();
                    int colon = pendingLaunch.Scene.IndexOf(":/");
                    if (colon >= 0)
                    {
                        var package = FileManager.GetPackage(pendingLaunch.Scene.Substring(0, colon));
                        if (package != null && !package.Enabled) { FinishLaunch("error", "package_disabled"); return; }
                    }
                    if (!FileManager.FileExists(pendingLaunch.Scene, false, false)) { FinishLaunch("error", "scene_unavailable"); return; }
                    // A rescan may take minutes. Revalidate the heartbeat before issuing a load.
                    if (!File.Exists(path)) { pendingLaunch = null; pendingLaunchFile = null; return; }
                    if (!SceneLaunchRequest.Fresh(SceneLaunchRequest.Milliseconds(File.GetLastWriteTimeUtc(path)), SceneLaunchRequest.Now))
                    { FinishLaunch("error", "invalid_request"); return; }
                    SceneLaunchRequest.Accept(path);
                    launchProgress = new LaunchProgress(readyController.LoadedSceneName, pendingLaunch.Scene, Time.unscaledTime);
                    readyController.Load(pendingLaunch.Scene);
                    // Some loads set this synchronously and finish before our next poll.
                    launchProgress.Observe(readyController.isLoading, readyController.LoadedSceneName, Time.unscaledTime);
                    return;
                }
                catch (Exception e)
                {
                    if (e is MissingMemberException || e is TypeLoadException) throw;
                    Logger.LogError(e);
                    string code = e is InvalidDataException ? "invalid_request" : "internal";
                    if (pendingLaunch != null) FinishLaunch("error", code, e.Message);
                    else SceneLaunchRequest.Complete(path, id, "error", code, e.Message);
                }
            }
        }

    }
}
