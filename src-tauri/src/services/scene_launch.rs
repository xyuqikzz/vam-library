//! Local, one-shot scene requests consumed by the bundled BepInEx plugin.
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

pub const HEADER: &str = "# VAM Library scene launch v1";
pub const TIMEOUT_MS: i64 = 180_000;
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
static LAUNCH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[cfg(windows)]
fn game_running(root: &Path) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    // No user-provided text is interpolated into PowerShell source.
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", "[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; $ErrorActionPreference='Stop'; @(Get-Process -Name VaM -ErrorAction SilentlyContinue | ForEach-Object { if (-not $_.Path) { throw 'Cannot read VaM process path' }; $_.Path }) | ConvertTo-Json -Compress"])
        .creation_flags(0x08000000).output().map_err(|e| format!("无法检查游戏进程: {e}"))?;
    if !output.status.success() {
        return Err("无法确认 VaM 进程所属实例，请退出游戏后重试".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return Ok(false);
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Paths {
        One(String),
        Many(Vec<String>),
    }
    let paths = match serde_json::from_str::<Paths>(text.trim()).map_err(|e| e.to_string())? {
        Paths::One(path) => vec![path],
        Paths::Many(paths) => paths,
    };
    let executable = fs::canonicalize(root.join("VaM.exe")).map_err(|e| e.to_string())?;
    let count = paths
        .iter()
        .filter(|p| fs::canonicalize(p).ok().as_ref() == Some(&executable))
        .count();
    if count > 1 {
        return Err("当前实例有多个 VaM 进程，请保留一个后重试".into());
    }
    Ok(count == 1)
}

#[cfg(not(windows))]
fn game_running(_: &Path) -> Result<bool, String> {
    Err("场景联动目前仅支持 Windows".into())
}

fn write_request(directory: &Path, scene: &str) -> Result<(PathBuf, PathBuf), String> {
    let now = chrono::Utc::now().timestamp_millis();
    let id = format!(
        "{:x}-{:x}-{:x}",
        std::process::id(),
        now,
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    );
    let request = directory.join(format!("{id}.request"));
    let response = directory.join(format!("{id}.result"));
    let temp = directory.join(format!("{id}.tmp"));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        write!(file, "{HEADER}\n{id}\n{now}\n{scene}\n")
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        drop(file);
        fs::rename(&temp, &request).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    Ok((request, response))
}

fn response_status(text: &str, id: &str) -> Result<bool, String> {
    let lines: Vec<_> = text.lines().collect();
    if lines.len() < 3 || lines[0] != HEADER || lines[1] != id {
        return Err("游戏返回了无效的场景联动结果".into());
    }
    match lines[2] {
        "loaded" => Ok(true),
        "error" => Err(lines.get(3).unwrap_or(&"游戏无法加载此场景").to_string()),
        _ => Err("未知的场景联动状态".into()),
    }
}

async fn wait_for_result(
    response: &Path,
    id: &str,
    timeout: Duration,
    mut child: Option<std::process::Child>,
) -> Result<(), String> {
    let started = Instant::now();
    loop {
        match fs::read_to_string(response) {
            Ok(text) => {
                response_status(&text, id)?;
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("读取场景加载结果失败: {e}")),
        }
        if let Some(child) = &mut child {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("VaM 已退出，场景未完成加载，请检查游戏启动日志".into());
            }
        }
        if started.elapsed() >= timeout {
            return Err("等待游戏加载场景超时。请检查游戏是否正常启动、模组是否已更新；若游戏仍在加载，请查看游戏窗口。".into());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub async fn launch(
    root: PathBuf,
    directory: PathBuf,
    scene: String,
    mode: String,
) -> Result<(), String> {
    let _guard = LAUNCH_LOCK
        .try_lock()
        .map_err(|_| "已有场景正在启动，请等待完成".to_string())?;
    let vr = match mode.as_str() {
        "desktop" => "None",
        "vr" => "OpenVR",
        _ => return Err("无效的游戏启动模式".into()),
    };
    let running = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        move || game_running(&root)
    })
    .await
    .map_err(|e| e.to_string())??;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let (request, response) = write_request(&directory, &scene)?;
    let id = request.file_stem().unwrap().to_str().unwrap();
    let result = async {
        let child = if !running {
            Some(
                std::process::Command::new(root.join("VaM.exe"))
                    .current_dir(&root)
                    .args(["-vrmode", vr])
                    .spawn()
                    .map_err(|e| format!("启动 VaM 失败: {e}"))?,
            )
        } else {
            None
        };
        wait_for_result(
            &response,
            id,
            Duration::from_millis(TIMEOUT_MS as u64),
            child,
        )
        .await
    }
    .await;
    // These two files belong exclusively to this invocation. Expired requests
    // must never replay on a later, unrelated game startup.
    let _ = fs::remove_file(request);
    let _ = fs::remove_file(response);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn delayed_game_acknowledgement_and_timeout_are_observed() {
        let directory = std::env::temp_dir().join(format!(
            "scene-ack-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&directory).unwrap();
        let (request, response) = write_request(&directory, "Saves/scene/Test.json").unwrap();
        let id = request.file_stem().unwrap().to_str().unwrap().to_string();
        assert!(wait_for_result(&response, &id, Duration::ZERO, None)
            .await
            .unwrap_err()
            .contains("超时"));
        let writer = tokio::spawn({
            let response = response.clone();
            let id = id.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(20)).await;
                let temp = response.with_extension("tmp");
                fs::write(&temp, format!("{HEADER}\n{id}\nloaded\n")).unwrap();
                fs::rename(temp, response).unwrap();
            }
        });
        wait_for_result(&response, &id, Duration::from_secs(5), None)
            .await
            .unwrap();
        writer.await.unwrap();
        fs::remove_file(request).unwrap();
        fs::remove_file(response).unwrap();
        fs::remove_dir(directory).unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn game_exit_is_reported_without_waiting_for_timeout() {
        use std::os::windows::process::CommandExt;
        let child = std::process::Command::new("cmd.exe")
            .args(["/C", "exit", "1"])
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        let response = std::env::temp_dir().join(format!(
            "missing-response-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        assert!(
            wait_for_result(&response, "abc", Duration::from_secs(5), Some(child))
                .await
                .unwrap_err()
                .contains("已退出")
        );
    }
    #[test]
    fn acknowledgements_are_correlated_and_errors_preserved() {
        assert!(response_status(&format!("{HEADER}\na\nloaded\n"), "a").unwrap());
        assert!(response_status(&format!("{HEADER}\nother\nloaded\n"), "a").is_err());
        assert_eq!(
            response_status(&format!("{HEADER}\na\nerror\n资源包已禁用\n"), "a").unwrap_err(),
            "资源包已禁用"
        );
        assert!(response_status("broken", "a").is_err());
    }
    #[test]
    fn requests_preserve_unicode_and_publish_only_complete_files() {
        let directory = std::env::temp_dir().join(format!(
            "scene-launch-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&directory).unwrap();
        let (request, response) =
            write_request(&directory, "作者.Scene.1:/Saves/scene/中文 场景.json").unwrap();
        let text = fs::read_to_string(&request).unwrap();
        assert!(text.ends_with("作者.Scene.1:/Saves/scene/中文 场景.json\n"));
        assert_eq!(text.lines().count(), 4);
        assert!(!response.exists());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_file(request).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
