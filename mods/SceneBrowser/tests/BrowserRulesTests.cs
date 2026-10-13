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
        Check(SceneLaunchRequest.Parse(launch, "abc-1", now, now).Scene == "作者.Scene.1:/Saves/scene/中文 场景.json", "Launch preserves exact package version and Unicode path");
        Check(SceneLaunchRequest.ValidScene("Saves/scene/Local.json"), "Local scene accepted");
        Check(SceneLaunchRequest.ValidScene("Author.Scene.With.Dots.2:/Saves/scene/a.json"), "Dotted package names retain exact numeric version");
        foreach (string invalid in new[] { "C:/Saves/scene/a.json", "Saves/scene/../a.json", "Saves/scene//a.json", "Saves\\scene\\a.json", "Saves/scene/a.json\nother", "Author.Scene.latest:/Saves/scene/a.json", "Custom/Atom/Person/Appearance/a.vap", "Saves/scene/a.json.fav" })
            Check(!SceneLaunchRequest.ValidScene(invalid), "Reject unsafe or non-scene path: " + invalid);
        foreach (string invalid in new[] { "../../a", "ABC", "", "abc\n1" }) Check(!SceneLaunchRequest.ValidId(invalid), "Reject unsafe request ID");
        foreach (long timestamp in new[] { now - SceneLaunchRequest.TimeoutMs - 1, now + 5001 })
        {
            bool expiredRejected = false;
            try { SceneLaunchRequest.Parse(launch.Replace(now.ToString(), timestamp.ToString()), "abc-1", now, now); }
            catch (InvalidDataException) { expiredRejected = true; }
            Check(expiredRejected, "Reject stale or future request");
        }
        // A cold start longer than v1's three minutes remains live with a heartbeat.
        Check(SceneLaunchRequest.Parse(launch.Replace(now.ToString(), (now - 600000).ToString()), "abc-1", now, now).Created == now - 600000, "Heartbeat extends slow cold start");
        foreach (long modified in new[] { now - 15001, now + 5001 })
        {
            bool rejectedHeartbeat = false;
            try { SceneLaunchRequest.Parse(launch, "abc-1", now, modified); } catch (InvalidDataException) { rejectedHeartbeat = true; }
            Check(rejectedHeartbeat, "Reject stopped or future heartbeats");
        }
        var progress = new LaunchProgress("same", "same", 0);
        Check(progress.Observe(false, "same", 1) == null, "Same scene must not succeed before loading starts");
        Check(progress.Observe(false, "same", 59) == null, "Allow delayed native confirmation");
        Check(progress.Observe(false, "same", 60) == "load_not_started", "Unanswered prompt produces specific error");
        progress = new LaunchProgress("same", "same", 0);
        Check(progress.Observe(true, "same", 1) == null && progress.Observe(false, "same", 2) == "loaded", "Reload succeeds only after loading transition");
        progress = new LaunchProgress("before", "wanted", 0);
        Check(progress.Observe(false, "wanted", 1) == "loaded", "Changed scene can complete between polls");
        progress = new LaunchProgress("before", "wanted", 0);
        Check(progress.Observe(true, "before", 1) == null && progress.Observe(false, "other", 2) == "load_failed", "Wrong final scene fails");
        Check(BrowserRules.CompareName("Scene 2", "Scene 10") < 0, "Natural digit ordering");
        Check(BrowserRules.CompareName("a01", "a1") < 0, "Leading zeros have deterministic ordinal tie break");
        Check(BrowserRules.CompareName("A2", "a10") < 0, "Case insensitive natural comparison");
        Check(BrowserRules.CompareName("a999999999999999999999", "a1000000000000000000000") < 0, "No integer overflow in names");
        Check(BrowserRules.CompareName("A", "a") < 0, "Case-only tie is a total order");
        Check(BrowserRules.ChildFolder("", "a/b/c") == "a", "Count deep descendants in direct child");
        Check(BrowserRules.ChildFolder("a", "a/b/c") == "a/b", "Nested direct child count");
        Check(BrowserRules.ChildFolder("a", "ab/c") == null && BrowserRules.ChildFolder("a", "a") == null, "Counts respect folder boundaries");
        Check(BrowserText.Mode("作者 A–Z") == "author" && BrowserText.Mode("Creator A–Z") == "author", "Both languages resolve stable sort IDs");
        string launchFile = Path.Combine(Path.GetTempPath(), "abc-" + Guid.NewGuid().ToString("N") + ".request");
        string launchId = Path.GetFileNameWithoutExtension(launchFile);
        string resultFile = Path.ChangeExtension(launchFile, ".result");
        try
        {
            File.WriteAllText(launchFile, launch);
            SceneLaunchRequest.Accept(launchFile);
            Check(File.Exists(Path.ChangeExtension(launchFile, ".accepted")), "Acceptance is atomically visible before completion");
            SceneLaunchRequest.Complete(launchFile, launchId, "loaded", "");
            Check(!File.Exists(launchFile), "Completed request consumed exactly once");
            Check(File.ReadAllText(resultFile).StartsWith(SceneLaunchRequest.Header + "\n" + launchId + "\nloaded\n"), "Completion correlated to request ID");
            SceneLaunchRequest.Complete(launchFile, launchId, "error", "should not replay");
            Check(!File.ReadAllText(resultFile).Contains("should not replay"), "Consumed request does not replay");
        }
        finally { if (File.Exists(launchFile)) File.Delete(launchFile); if (File.Exists(resultFile)) File.Delete(resultFile); File.Delete(Path.ChangeExtension(launchFile, ".accepted")); }
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
        Check(BrowserRules.CompareSize(1000, 100, true, "Z", "z", "1", "A", "a", "2") < 0, "Largest package precedes author and scene name");
        Check(BrowserRules.CompareSize(100, 1000, false, "Z", "z", "1", "A", "a", "2") < 0, "Smallest package precedes author and scene name");
        Check(BrowserRules.CompareSize(long.MaxValue, 0, true, "A", "a", "1", "A", "a", "2") < 0, "Large package sizes do not overflow descending comparison");
        Check(BrowserRules.CompareSize(0, long.MaxValue, false, "A", "a", "1", "A", "a", "2") < 0, "Zero-byte packages sort first ascending without overflow");
        foreach (bool descending in new[] { false, true })
        {
            Check(BrowserRules.CompareSize(null, 0, descending, null, "a", "1", "Z", "z", "2") > 0, "Loose scenes follow even empty packages in both directions");
            Check(BrowserRules.CompareSize(0, null, descending, "Z", "z", "1", null, "a", "2") < 0, "Known package size precedes loose scenes in both directions");
            Check(BrowserRules.CompareSize(100, 100, descending, "alice", "a", "1", "ALICE", "z", "2") < 0, "Same-package scenes retain name ordering in both directions");
            Check(BrowserRules.CompareSize(100, 100, descending, "A", "a", "1", "A", "a", "2") < 0, "Equal size and scene name use deterministic path ordering");
            Check(BrowserRules.CompareSize(null, null, descending, null, "a", "1", null, "z", "2") < 0, "Loose scenes retain deterministic ordering");
        }
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
        string cleanup = Path.Combine(Path.GetTempPath(), "LaunchCleanup-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(cleanup);
        try
        {
            foreach (string ext in new[] { ".request", ".result", ".accepted", ".tmp", ".keep" })
            {
                string file = Path.Combine(cleanup, "old" + ext); File.WriteAllText(file, ""); File.SetLastWriteTimeUtc(file, DateTime.UtcNow.AddMinutes(-31));
                File.WriteAllText(Path.Combine(cleanup, "new" + ext), "");
            }
            SceneLaunchRequest.Cleanup(cleanup, SceneLaunchRequest.Now);
            Check(Directory.GetFiles(cleanup).Length == 6 && File.Exists(Path.Combine(cleanup, "old.keep")), "Cleanup removes only stale protocol files");
            string settings = Path.Combine(cleanup, "settings");
            File.WriteAllText(settings, "# VAM Library app settings v1\nlanguage\ten-US\n"); BrowserText.Refresh(settings);
            Check(BrowserText.Get("folders") == "Browse by folder", "Settings select English labels");
            File.WriteAllText(settings, "broken"); BrowserText.Refresh(settings);
            Check(BrowserText.Get("folders") == "Browse by folder", "Malformed settings retain last valid language");
            File.Delete(settings); BrowserText.Refresh(settings);
            Check(BrowserText.Get("folders") == "按文件夹浏览", "Missing settings restore default language");
        }
        finally { foreach (string file in Directory.GetFiles(cleanup)) File.Delete(file); Directory.Delete(cleanup); }
        Console.WriteLine("PASS: " + checks + " browser rules checks");
    }
}
