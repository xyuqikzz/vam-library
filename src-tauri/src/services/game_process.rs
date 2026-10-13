//! Fresh native process snapshots shared by resource operations and the installer.
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(windows)]
pub fn game_running(root: &Path) -> Result<bool, String> {
    running_from_paths(root, &process_probe::paths("VaM.exe")?)
}

#[cfg(windows)]
fn running_from_paths(root: &Path, paths: &[PathBuf]) -> Result<bool, String> {
    if paths.is_empty() {
        return Ok(false);
    }
    let executable = fs::canonicalize(root.join("VaM.exe")).map_err(|e| e.to_string())?;
    let mut count = 0;
    for path in paths {
        // An unreadable process path cannot establish that the game is stopped.
        if fs::canonicalize(path).map_err(|e| e.to_string())? == executable {
            count += 1;
        }
    }
    if count > 1 {
        return Err("当前游戏目录有多个 VaM 进程，请保留一个后重试".into());
    }
    Ok(count == 1)
}

#[cfg(windows)]
mod process_probe {
    use std::{ffi::OsString, mem::size_of, os::windows::ffi::OsStringExt, path::PathBuf};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn error() -> String {
        format!(
            "无法确认 VaM 进程所属游戏目录，请退出游戏后重试：{}",
            std::io::Error::last_os_error()
        )
    }
    pub(super) fn paths(expected: &str) -> Result<Vec<PathBuf>, String> {
        // Enumerate names without opening every process. Query matching images
        // with limited-information access; no shell startup or cached results.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if raw == INVALID_HANDLE_VALUE {
            return Err(error());
        }
        let snapshot = Handle(raw);
        // PROCESSENTRY32W contains only integer fields and a UTF-16 array.
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        let mut available = unsafe { Process32FirstW(snapshot.0, &mut entry) };
        let mut paths = Vec::new();
        while available != 0 {
            let length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..length]).eq_ignore_ascii_case(expected) {
                let raw = unsafe {
                    OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID)
                };
                if raw.is_null() {
                    // The process may have exited since the snapshot. Access
                    // denial and every other failure must still block cleanup.
                    if std::io::Error::last_os_error().raw_os_error() != Some(87) {
                        return Err(error());
                    }
                } else {
                    let process = Handle(raw);
                    let mut buffer = vec![0u16; 32768];
                    let mut length = buffer.len() as u32;
                    if unsafe {
                        QueryFullProcessImageNameW(process.0, 0, buffer.as_mut_ptr(), &mut length)
                    } == 0
                    {
                        return Err(error());
                    }
                    paths.push(PathBuf::from(OsString::from_wide(
                        &buffer[..length as usize],
                    )));
                }
            }
            available = unsafe { Process32NextW(snapshot.0, &mut entry) };
        }
        if std::io::Error::last_os_error().raw_os_error() != Some(18) {
            return Err(error());
        }
        Ok(paths)
    }
}

#[cfg(not(windows))]
pub fn game_running(_: &Path) -> Result<bool, String> {
    Err("场景联动目前仅支持 Windows".into())
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn native_process_probe_finds_current_image_and_absent_names() {
        let current = std::env::current_exe().unwrap();
        let started = std::time::Instant::now();
        let paths =
            super::process_probe::paths(current.file_name().unwrap().to_str().unwrap()).unwrap();
        assert!(
            paths
                .iter()
                .any(|p| std::fs::canonicalize(p).unwrap()
                    == std::fs::canonicalize(&current).unwrap())
        );
        assert!(
            super::process_probe::paths("vam-library-nonexistent-test-process.exe")
                .unwrap()
                .is_empty()
        );
        eprintln!(
            "Native process checks (2 snapshots): {:?}",
            started.elapsed()
        );
    }
    #[cfg(windows)]
    #[test]
    fn process_check_distinguishes_instances_and_fails_closed() {
        use crate::services::resource_files::tests::TestDir;
        let first = TestDir::new();
        let second = TestDir::new();
        let a = first.write("VaM.exe", b"fixture");
        let b = second.write("VaM.exe", b"fixture");
        assert!(!super::running_from_paths(&first.0, &[]).unwrap());
        assert!(!super::running_from_paths(&first.0, &[b.clone()]).unwrap());
        assert!(super::running_from_paths(&first.0, &[a.clone(), b]).unwrap());
        assert!(super::running_from_paths(&first.0, &[a.clone(), a]).is_err());
        assert!(super::running_from_paths(&first.0, &[first.0.join("missing.exe")]).is_err());
    }
}
