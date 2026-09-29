using System;
using System.IO;
using System.Runtime.InteropServices;

// Isolated process hosting the exact Mono shipped with VaM, not the Windows CLR.
class MonoHost
{
    [DllImport("kernel32", CharSet = CharSet.Unicode)] static extern bool SetDllDirectory(string path);
    [DllImport("mono", CallingConvention = CallingConvention.Cdecl)] static extern void mono_set_assemblies_path(string path);
    [DllImport("mono", CallingConvention = CallingConvention.Cdecl)] static extern void mono_set_dirs(string lib, string etc);
    [DllImport("mono", CallingConvention = CallingConvention.Cdecl)] static extern IntPtr mono_jit_init_version(string name, string version);
    [DllImport("mono", CallingConvention = CallingConvention.Cdecl)] static extern IntPtr mono_domain_assembly_open(IntPtr domain, string path);
    [DllImport("mono", CallingConvention = CallingConvention.Cdecl)] static extern int mono_jit_exec(IntPtr domain, IntPtr assembly, int argc, IntPtr argv);

    static int Main(string[] args)
    {
        string root = Path.GetFullPath(args[0]);
        SetDllDirectory(Path.Combine(root, "Mono/EmbedRuntime"));
        mono_set_dirs(Path.Combine(root, "Mono"), Path.Combine(root, "Mono/etc"));
        mono_set_assemblies_path(Path.Combine(root, "VaM_Data/Managed") + ";" + Path.Combine(root, "BepInEx/core"));
        IntPtr domain = mono_jit_init_version("SceneBrowserRegression", "v2.0.50727");
        if (domain == IntPtr.Zero) throw new Exception("Cannot initialize game Mono");
        IntPtr assembly = mono_domain_assembly_open(domain, Path.GetFullPath(args[1]));
        if (assembly == IntPtr.Zero) throw new Exception("Cannot open runtime probe");
        string[] managedArgs = { Path.GetFullPath(args[1]), root, Path.GetFullPath(args[2]) };
        IntPtr argv = Marshal.AllocHGlobal(IntPtr.Size * managedArgs.Length);
        for (int i = 0; i < managedArgs.Length; i++) Marshal.WriteIntPtr(argv, i * IntPtr.Size, Marshal.StringToHGlobalAnsi(managedArgs[i]));
        try { return mono_jit_exec(domain, assembly, managedArgs.Length, argv); }
        finally
        {
            for (int i = 0; i < managedArgs.Length; i++) Marshal.FreeHGlobal(Marshal.ReadIntPtr(argv, i * IntPtr.Size));
            Marshal.FreeHGlobal(argv);
        }
    }
}
