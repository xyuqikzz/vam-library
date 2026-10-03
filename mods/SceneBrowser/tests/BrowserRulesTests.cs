using System;
using System.IO;
using VamLibrary.SceneBrowser;

class BrowserRulesTests
{
    static int checks;
    static void Check(bool condition, string message)
    {
        if (!condition) throw new Exception(message);
        checks++;
    }
    static void Main()
    {
        long now = SceneLaunchRequest.Now;
        string launch = SceneLaunchRequest.Header + "\nabc-1\n" + now + "\n作者.Scene.1:/Saves/scene/中文 场景.json\n";
        Check(SceneLaunchRequest.Parse(launch, "abc-1", now).Scene == "作者.Scene.1:/Saves/scene/中文 场景.json", "Launch preserves exact package version and Unicode path");
        Check(SceneLaunchRequest.ValidScene("Saves/scene/Local.json"), "Local scene accepted");
        Check(SceneLaunchRequest.ValidScene("Author.Scene.With.Dots.2:/Saves/scene/a.json"), "Dotted package names retain exact numeric version");
        foreach (string invalid in new[] { "C:/Saves/scene/a.json", "Saves/scene/../a.json", "Saves/scene//a.json", "Saves\\scene\\a.json", "Saves/scene/a.json\nother", "Author.Scene.latest:/Saves/scene/a.json", "Custom/Atom/Person/Appearance/a.vap", "Saves/scene/a.json.fav" })
            Check(!SceneLaunchRequest.ValidScene(invalid), "Reject unsafe or non-scene path: " + invalid);
        foreach (string invalid in new[] { "../../a", "ABC", "", "abc\n1" }) Check(!SceneLaunchRequest.ValidId(invalid), "Reject unsafe request ID");
        foreach (long timestamp in new[] { now - SceneLaunchRequest.TimeoutMs - 1, now + 5001 })
        {
            bool expiredRejected = false;
            try { SceneLaunchRequest.Parse(launch.Replace(now.ToString(), timestamp.ToString()), "abc-1", now); }
            catch (InvalidDataException) { expiredRejected = true; }
            Check(expiredRejected, "Reject stale or future request");
        }
        string launchFile = Path.Combine(Path.GetTempPath(), "abc-" + Guid.NewGuid().ToString("N") + ".request");
        string launchId = Path.GetFileNameWithoutExtension(launchFile);
        string resultFile = Path.ChangeExtension(launchFile, ".result");
        try
        {
            File.WriteAllText(launchFile, launch);
            SceneLaunchRequest.Complete(launchFile, launchId, "loaded", "");
            Check(!File.Exists(launchFile), "Completed request consumed exactly once");
            Check(File.ReadAllText(resultFile).StartsWith(SceneLaunchRequest.Header + "\n" + launchId + "\nloaded\n"), "Completion correlated to request ID");
            SceneLaunchRequest.Complete(launchFile, launchId, "error", "should not replay");
            Check(!File.ReadAllText(resultFile).Contains("should not replay"), "Consumed request does not replay");
        }
        finally { if (File.Exists(launchFile)) File.Delete(launchFile); if (File.Exists(resultFile)) File.Delete(resultFile); }
        Check(BrowserRules.Compare("Alice", "z", "1", "Bob", "a", "2") < 0, "Author precedes scene title");
        Check(BrowserRules.Compare("alice", "a", "1", "ALICE", "z", "2") < 0, "Case-insensitive author grouping");
        Check(BrowserRules.Compare(null, "a", "1", "Bob", "z", "2") > 0, "Loose scenes last");
        Check(BrowserRules.Compare("Alice", "a", "1", "Alice", "a", "2") < 0, "Deterministic tie break");
        Check(BrowserRules.CompareImported(2000, 1000, "Z", "z", "1", "A", "a", "2") < 0, "Newest import precedes author and scene name");
        Check(BrowserRules.CompareImported(null, 1000, "A", "a", "1", "Z", "z", "2") > 0, "Unknown import time follows known times");
        Check(BrowserRules.CompareImported(1000, null, "Z", "z", "1", "A", "a", "2") < 0, "Known import time precedes unknown times");
        Check(BrowserRules.CompareImported(1000, 1000, "alice", "a", "1", "ALICE", "z", "2") < 0, "Same-package scenes use scene-name tie break");
        Check(BrowserRules.CompareImported(null, null, "A", "a", "1", "B", "a", "2") < 0, "Unindexed scenes retain deterministic author sorting");
        Check(BrowserRules.CompareImported(1000, 1000, "A", "a", "1", "A", "a", "2") < 0, "Equal timestamps and names use full path");
        string timesFile = Path.Combine(Path.GetTempPath(), "VamImportTimes-" + Guid.NewGuid().ToString("N") + ".tsv");
        try
        {
            var index = new ImportTimeIndex();
            Check(!index.Refresh(timesFile, false) && !index.Available && index.Find("A.B.1") == null, "Missing snapshot keeps all scenes unindexed");
            File.WriteAllText(timesFile, ImportTimeIndex.Header + "\n作者.Scene.1\t1790827200000\nALICE.Scene.1\t2000\nalice.scene.1\t1000\n");
            Check(index.Refresh(timesFile, false) && index.Available && index.Count == 2, "Load a complete Unicode snapshot");
            Check(index.Find("作者.Scene.1") == 1790827200000L, "UTC milliseconds are preserved");
            Check(index.Find("Alice.Scene.1") == 1000, "UID matching ignores case and retains earliest duplicate time");
            Check(index.Find("Alice.Scene.2") == null, "Package versions have independent import times");
            Check(!index.Refresh(timesFile, false), "Unchanged snapshot does not trigger a refresh");
            File.WriteAllText(timesFile, ImportTimeIndex.Header + "\nNew.Scene.1\t3000\n");
            Check(index.Refresh(timesFile, true) && index.Count == 1 && index.Find("Alice.Scene.1") == null, "Replacement removes stale records");
            File.WriteAllText(timesFile, ImportTimeIndex.Header + "\nNew.Scene.1\tbroken\n");
            Check(!index.Refresh(timesFile, true) && index.Error != null && index.Find("New.Scene.1") == 3000, "Invalid snapshot retains last valid data");
            File.WriteAllText(timesFile, ImportTimeIndex.Header + "\r\n");
            Check(index.Refresh(timesFile, true) && index.Count == 0 && index.Error == null, "Empty library clears timestamps and read error");
            File.Delete(timesFile);
            Check(index.Refresh(timesFile, false) && !index.Available, "Deleted snapshot clears availability");
            foreach (string invalid in new[] { "wrong header\n", ImportTimeIndex.Header + "\nA.B.1\t-1\n", ImportTimeIndex.Header + "\nA\tB\t1000\n" })
            {
                bool bad = false;
                try { ImportTimeIndex.Parse(invalid); } catch (FormatException) { bad = true; }
                Check(bad, "Reject invalid snapshot formats");
            }
        }
        finally { if (File.Exists(timesFile)) File.Delete(timesFile); }
        string root = Path.Combine(Path.GetTempPath(), "ModRules", "AddonPackages");
        Check(BrowserRules.PackageFolder(root, Path.Combine(root, "Author.Scene.1.var")) == "", "Root packages");
        Check(BrowserRules.PackageFolder(root, Path.Combine(root, "作者", "子目录", "A.B.1.var")) == "作者/子目录", "Nested Unicode folders");
        Check(BrowserRules.PackageFolder(root, Path.Combine(root + "Other", "A.B.1.var")) == null, "Directory boundary");
        Check(BrowserRules.PackageFolder(root, null) == null, "No package path");
        Check(BrowserRules.PackageFolder(root, Path.Combine(root, "..", "external.var")) == null, "Outside resource");
        Check(BrowserRules.ResolveFolder(root, "") == Path.GetFullPath(root), "Root navigation");
        Check(BrowserRules.ResolveFolder(root, "a/b") == Path.Combine(root, "a", "b"), "Nested navigation");
        bool rejected = false;
        try { BrowserRules.ResolveFolder(root, "../outside"); } catch (ArgumentException) { rejected = true; }
        Check(rejected, "Traversal rejected");
        string fixture = Path.Combine(Path.GetTempPath(), "VamFolders-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Path.Combine(fixture, "场景/子目录"));
        Directory.CreateDirectory(Path.Combine(fixture, "其他"));
        Directory.CreateDirectory(Path.Combine(fixture, ".VAMBoxLibrary-downloads"));
        try
        {
            var folders = BrowserRules.ListFolders(fixture, "");
            Check(folders.Count == 2, "Root with no VAR files still exposes child folders, excludes internal download folder");
            Check(folders.Exists(f => f.RelativePath == "场景"), "Scene folder appears in preview navigation");
            var scene = BrowserRules.ListFolders(fixture, "场景");
            Check(scene.Count == 3 && scene[0].RelativePath == "" && scene[1].RelativePath == "", "Root and parent links precede children");
            Check(scene[2].RelativePath == "场景/子目录", "Nested folder click target");
            var empty = BrowserRules.ListFolders(fixture, "场景/子目录");
            Check(empty.Count == 2 && empty[1].RelativePath == "场景", "Empty folder retains return navigation");
            Check(BrowserRules.PackageFolder(fixture, Path.Combine(fixture, "场景/A.B.1.var")) == "场景", "Scene visibility matches entered folder");
        }
        finally
        {
            Directory.Delete(Path.Combine(fixture, "场景/子目录"));
            Directory.Delete(Path.Combine(fixture, "场景"));
            Directory.Delete(Path.Combine(fixture, "其他"));
            Directory.Delete(Path.Combine(fixture, ".VAMBoxLibrary-downloads"));
            Directory.Delete(fixture);
        }
        Console.WriteLine("PASS: " + checks + " browser rules checks");
    }
}
