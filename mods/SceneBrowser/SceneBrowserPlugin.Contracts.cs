using System;
using System.Collections.Generic;
using System.Reflection;
using HarmonyLib;
using MVR.FileManagement;
using uFileBrowser;
using Item = uFileBrowser.FileBrowser.FileAndDirInfo;

namespace VamLibrary.SceneBrowser
{
    public sealed partial class SceneBrowserPlugin
    {
        private static void Property(Type type, string name, Type expected)
        {
            var property = AccessTools.Property(type, name);
            if (property == null || property.PropertyType != expected || property.GetGetMethod() == null)
                throw new MissingMemberException(type.FullName, name);
        }
        private static void Method(Type type, string name, Type result, bool isStatic, params Type[] args)
        {
            var method = AccessTools.Method(type, name, args);
            if (method == null || method.ReturnType != result || method.IsStatic != isStatic)
                throw new MissingMethodException(type.FullName, name);
        }
        private static void CheckBrowserContract()
        {
            if (Sorted == null || Sorted.FieldType != typeof(List<Item>) ||
                Displayed == null || Displayed.FieldType != typeof(HashSet<FileButton>) ||
                DirectoryEntryProperty == null || DirectoryEntryProperty.GetSetMethod(true) == null)
                throw new MissingMemberException("Unsupported VaM scene browser fields");
            Method(typeof(FileBrowser), "HideButton", typeof(void), false, typeof(FileButton));
            Method(typeof(FileBrowser), "SyncSort", typeof(void), false);
            Method(typeof(FileBrowser), "CreateFileButton", typeof(FileButton), false,
                typeof(string), typeof(string), typeof(bool), typeof(bool), typeof(bool), typeof(bool), typeof(bool), typeof(bool), typeof(bool));
            Method(typeof(FileBrowser), "SetTitle", typeof(void), false, typeof(string));
            Method(typeof(FileBrowser), "SetSortBy", typeof(void), false, typeof(string));
            Method(typeof(FileBrowser), "SortFilesAndDirs", typeof(void), false, typeof(List<Item>));
            Method(typeof(FileBrowser), "OnFileClick", typeof(void), false, typeof(FileButton));
            foreach (string name in new[] { "ShowInternal", "Hide", "SyncDisplayed" })
                if (AccessTools.Method(typeof(FileBrowser), name) == null) throw new MissingMethodException(typeof(FileBrowser).FullName, name);
            Property(typeof(VarFileEntry), "Package", typeof(VarPackage));
            Property(typeof(VarPackage), "Size", typeof(long));
            foreach (string name in new[] { "Creator", "Uid", "FullPath" }) Property(typeof(VarPackage), name, typeof(string));
        }
        private static void CheckLaunchContract()
        {
            Method(typeof(SuperController), "Start", typeof(void), false);
            Method(typeof(SuperController), "Load", typeof(void), false, typeof(string));
            Method(typeof(SuperController), "RescanPackages", typeof(void), false);
            Property(typeof(SuperController), "isLoading", typeof(bool));
            Property(typeof(SuperController), "LoadedSceneName", typeof(string));
            Method(typeof(FileManager), "FileExists", typeof(bool), true, typeof(string), typeof(bool), typeof(bool));
            Method(typeof(FileManager), "GetPackage", typeof(VarPackage), true, typeof(string));
            Property(typeof(VarPackage), "Enabled", typeof(bool));
        }
    }
}
