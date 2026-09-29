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
        Check(BrowserRules.Compare("Alice", "z", "1", "Bob", "a", "2") < 0, "Author precedes scene title");
        Check(BrowserRules.Compare("alice", "a", "1", "ALICE", "z", "2") < 0, "Case-insensitive author grouping");
        Check(BrowserRules.Compare(null, "a", "1", "Bob", "z", "2") > 0, "Loose scenes last");
        Check(BrowserRules.Compare("Alice", "a", "1", "Alice", "a", "2") < 0, "Deterministic tie break");
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
