using System;
using System.IO;
using System.Collections.Generic;

namespace VamLibrary.SceneBrowser
{
    public static class BrowserRules
    {
        public sealed class FolderLink
        {
            public string Label;
            public string RelativePath;
            public FolderLink(string label, string relativePath) { Label = label; RelativePath = relativePath; }
        }

        public static List<FolderLink> ListFolders(string root, string relative)
        {
            string path = ResolveFolder(root, relative);
            var result = new List<FolderLink>();
            // Navigation stays available even when this folder contains no scenes.
            if (relative.Length > 0)
            {
                result.Add(new FolderLink("返回 AddonPackages 根目录", ""));
                int slash = relative.LastIndexOf('/');
                result.Add(new FolderLink(".. / 返回上一级", slash < 0 ? "" : relative.Substring(0, slash)));
            }
            string[] children = Directory.GetDirectories(path);
            Array.Sort(children, StringComparer.OrdinalIgnoreCase);
            foreach (string child in children)
            {
                string name = Path.GetFileName(child);
                if (name.StartsWith(".", StringComparison.Ordinal) || (File.GetAttributes(child) & FileAttributes.ReparsePoint) != 0) continue;
                result.Add(new FolderLink("[文件夹] " + name, relative.Length == 0 ? name : relative + "/" + name));
            }
            return result;
        }
        public static int Compare(string authorA, string nameA, string pathA,
                                  string authorB, string nameB, string pathB)
        {
            // Loose scenes have no package author and follow the named creators.
            int result = string.IsNullOrEmpty(authorA).CompareTo(string.IsNullOrEmpty(authorB));
            if (result == 0) result = StringComparer.OrdinalIgnoreCase.Compare(authorA, authorB);
            if (result == 0) result = StringComparer.OrdinalIgnoreCase.Compare(nameA, nameB);
            if (result == 0) result = StringComparer.Ordinal.Compare(pathA, pathB);
            return result;
        }

        public static string PackageFolder(string root, string packagePath)
        {
            if (string.IsNullOrEmpty(packagePath)) return null;
            string basePath = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            string fullPath = Path.GetFullPath(packagePath);
            string prefix = basePath + Path.DirectorySeparatorChar;
            if (!fullPath.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)) return null;
            string relative = fullPath.Substring(prefix.Length).Replace('\\', '/');
            int slash = relative.LastIndexOf('/');
            return slash < 0 ? "" : relative.Substring(0, slash);
        }

        public static string ResolveFolder(string root, string relative)
        {
            string basePath = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            string path = Path.GetFullPath(Path.Combine(basePath, relative));
            if (!path.Equals(basePath, StringComparison.OrdinalIgnoreCase) &&
                !path.StartsWith(basePath + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
                throw new ArgumentException("Folder must be inside AddonPackages.");
            return path;
        }
    }
}
