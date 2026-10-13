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
            public bool Navigation;
            public FolderLink(string label, string relativePath, bool navigation = false) { Label = label; RelativePath = relativePath; Navigation = navigation; }
        }

        public static List<FolderLink> ListFolders(string root, string relative)
        {
            string path = ResolveFolder(root, relative);
            var result = new List<FolderLink>();
            // Navigation stays available even when this folder contains no scenes.
            if (relative.Length > 0)
            {
                result.Add(new FolderLink(BrowserText.Get("root"), "", true));
                int slash = relative.LastIndexOf('/');
                result.Add(new FolderLink(BrowserText.Get("parent"), slash < 0 ? "" : relative.Substring(0, slash), true));
            }
            string[] children = Directory.GetDirectories(path);
            Array.Sort(children, delegate(string a, string b) { return CompareName(Path.GetFileName(a), Path.GetFileName(b)); });
            foreach (string child in children)
            {
                string name = Path.GetFileName(child);
                if (name.StartsWith(".", StringComparison.Ordinal)) continue;
                result.Add(new FolderLink(BrowserText.Get("folder") + name, relative.Length == 0 ? name : relative + "/" + name));
            }
            return result;
        }
        public static int Compare(string authorA, string nameA, string pathA,
                                  string authorB, string nameB, string pathB)
        {
            // Loose scenes have no package author and follow the named creators.
            int result = string.IsNullOrEmpty(authorA).CompareTo(string.IsNullOrEmpty(authorB));
            if (result == 0) result = StringComparer.OrdinalIgnoreCase.Compare(authorA, authorB);
            if (result == 0) result = CompareName(nameA, nameB);
            if (result == 0) result = StringComparer.Ordinal.Compare(pathA, pathB);
            return result;
        }

        public static int CompareImported(long? timeA, long? timeB, string authorA, string nameA, string pathA,
                                          string authorB, string nameB, string pathB)
        {
            int result = timeB.HasValue.CompareTo(timeA.HasValue);
            if (result == 0 && timeA.HasValue) result = timeB.Value.CompareTo(timeA.Value);
            return result != 0 ? result : Compare(authorA, nameA, pathA, authorB, nameB, pathB);
        }

        public static int CompareSize(long? sizeA, long? sizeB, bool descending,
                                      string authorA, string nameA, string pathA,
                                      string authorB, string nameB, string pathB)
        {
            // Loose scenes stay last in both directions. Use CompareTo to avoid overflow on large VARs.
            int result = sizeB.HasValue.CompareTo(sizeA.HasValue);
            if (result == 0 && sizeA.HasValue)
                result = descending ? sizeB.Value.CompareTo(sizeA.Value) : sizeA.Value.CompareTo(sizeB.Value);
            return result != 0 ? result : Compare(authorA, nameA, pathA, authorB, nameB, pathB);
        }

        public static int CompareName(string a, string b)
        {
            a = a ?? ""; b = b ?? "";
            int i = 0, j = 0;
            while (i < a.Length && j < b.Length)
            {
                if (a[i] >= '0' && a[i] <= '9' && b[j] >= '0' && b[j] <= '9')
                {
                    int endA = i, endB = j;
                    while (endA < a.Length && a[endA] >= '0' && a[endA] <= '9') endA++;
                    while (endB < b.Length && b[endB] >= '0' && b[endB] <= '9') endB++;
                    while (i < endA && a[i] == '0') i++;
                    while (j < endB && b[j] == '0') j++;
                    int result = (endA - i).CompareTo(endB - j);
                    if (result != 0) return result;
                    while (i < endA) { result = a[i++].CompareTo(b[j++]); if (result != 0) return result; }
                    i = endA; j = endB;
                }
                else
                {
                    int result = Char.ToUpperInvariant(a[i++]).CompareTo(Char.ToUpperInvariant(b[j++]));
                    if (result != 0) return result;
                }
            }
            int remaining = (a.Length - i).CompareTo(b.Length - j);
            return remaining != 0 ? remaining : StringComparer.Ordinal.Compare(a, b);
        }

        // Return the immediate child containing this package, including descendants.
        public static string ChildFolder(string current, string folder)
        {
            string prefix = current.Length == 0 ? "" : current + "/";
            if (folder == null || !folder.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) || folder.Length <= prefix.Length) return null;
            int slash = folder.IndexOf('/', prefix.Length);
            return slash < 0 ? folder : folder.Substring(0, slash);
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
