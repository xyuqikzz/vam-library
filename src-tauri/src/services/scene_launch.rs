//! Local, one-shot scene requests consumed by the bundled BepInEx plugin.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use super::game_process::game_running;
use serde::Serialize;
pub const HEADER: &str = "# VAM Library scene launch v2";
const HEARTBEAT: Duration = Duration::from_secs(5);

#[derive(Debug, Serialize)]
pub struct LaunchError {
    pub code: String,
    pub detail: Option<String>,
}
impl LaunchError {
    pub fn new(code: &str, detail: Option<String>) -> Self {
        Self {
            code: code.into(),
            detail,
        }
    }
}
impl From<String> for LaunchError {
    fn from(detail: String) -> Self {
        if detail == "launch_unavailable" {
            Self::new(&detail, None)
        } else {
            Self::new("internal", Some(detail))
        }
    }
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LaunchOutcome {
    Loaded,
    StillLoading,
}

#[derive(Clone, Copy)]
struct WaitLimits {
    acceptance: Duration,
    loading: Duration,
}
impl WaitLimits {
    fn for_game(running: bool) -> Self {
        Self {
            acceptance: Duration::from_secs(if running { 120 } else { 900 }),
            loading: Duration::from_secs(600),
        }
    }
}

struct RequestFiles(PathBuf);
impl Drop for RequestFiles {
    fn drop(&mut self) {
        for extension in [
            "request",
            "result",
            "accepted",
            "tmp",
            "result.tmp",
            "accepted.tmp",
        ] {
            let _ = fs::remove_file(self.0.with_extension(extension));
        }
    }
}
static NEXT_ID: AtomicU64 = AtomicU64::new(0);
static LAUNCH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub fn ensure_game_stopped(root: &Path) -> Result<(), String> {
    if game_running(root)? {
        return Err("请先关闭当前实例的 VaM，再移动或恢复资源".into());
    }
    Ok(())
}

/// For background work: true when VaM of this instance runs or that cannot be
/// ruled out. Windows uses a fresh native process snapshot.
pub fn game_may_be_running(root: &Path) -> bool {
    #[cfg(windows)]
    {
        !matches!(game_running(root), Ok(false))
    }
    #[cfg(not(windows))]
    {
        let _ = root;
        false
    }
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

fn response_status(text: &str, id: &str) -> Result<LaunchOutcome, LaunchError> {
    let lines: Vec<_> = text.lines().collect();
    if lines.len() < 3 || lines.len() > 5 || lines[0] != HEADER || lines[1] != id {
        return Err(LaunchError::new("invalid_response", None));
    }
    match lines[2] {
        "loaded" => Ok(LaunchOutcome::Loaded),
        "error" => {
            let code = lines.get(3).copied().unwrap_or("internal");
            if ![
                "invalid_request",
                "scene_unavailable",
                "package_disabled",
                "load_not_started",
                "load_failed",
                "internal",
            ]
            .contains(&code)
            {
                return Err(LaunchError::new("invalid_response", None));
            }
            Err(LaunchError::new(
                code,
                lines
                    .get(4)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string()),
            ))
        }
        _ => Err(LaunchError::new("invalid_response", None)),
    }
}

async fn wait_for_result(
    request: &Path,
    id: &str,
    limits: WaitLimits,
    mut child: Option<std::process::Child>,
    mut progress: impl FnMut(&str),
) -> Result<LaunchOutcome, LaunchError> {
    let started = Instant::now();
    let mut accepted_at = None;
    let mut heartbeat_at = Instant::now();
    let response = request.with_extension("result");
    let accepted = request.with_extension("accepted");
    loop {
        match fs::read_to_string(&response) {
            Ok(text) => return response_status(&text, id),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string().into()),
        }
        if let Some(child) = &mut child {
            if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err(LaunchError::new("game_exited", None));
            }
        }
        if accepted_at.is_none() && accepted.is_file() {
            accepted_at = Some(Instant::now());
            progress("loading");
        }
        if let Some(accepted_at) = accepted_at {
            if accepted_at.elapsed() >= limits.loading {
                return Ok(LaunchOutcome::StillLoading);
            }
        } else if started.elapsed() >= limits.acceptance {
            return Err(LaunchError::new("accept_timeout", None));
        }
        if heartbeat_at.elapsed() >= HEARTBEAT {
            // Never recreate a consumed request. A result may have been published
            // between our read and this heartbeat; the next iteration will see it.
            match fs::OpenOptions::new().write(true).open(request) {
                Ok(file) => file
                    .set_modified(std::time::SystemTime::now())
                    .map_err(|e| e.to_string())?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string().into()),
            }
            heartbeat_at = Instant::now();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

pub async fn launch(
    root: PathBuf,
    directory: PathBuf,
    scene: String,
    mode: String,
    on_progress: Option<tauri::ipc::Channel<String>>,
) -> Result<LaunchOutcome, LaunchError> {
    let _guard = LAUNCH_LOCK
        .try_lock()
        .map_err(|_| LaunchError::new("busy", None))?;
    let vr = match mode.as_str() {
        "desktop" => "None",
        "vr" => "OpenVR",
        _ => return Err(LaunchError::new("invalid_request", None)),
    };
    let running = tauri::async_runtime::spawn_blocking({
        let root = root.clone();
        move || game_running(&root)
    })
    .await
    .map_err(|e| e.to_string())??;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let (request, _) = write_request(&directory, &scene)?;
    let _cleanup = RequestFiles(request.clone());
    let id = request.file_stem().unwrap().to_str().unwrap();
    let progress = |phase: &str| {
        if let Some(channel) = &on_progress {
            let _ = channel.send(phase.to_string());
        }
    };
    progress(if running { "sending" } else { "opening" });
    let child = if !running {
        Some(
            std::process::Command::new(root.join("VaM.exe"))
                .current_dir(&root)
                .args(["-vrmode", vr])
                .spawn()
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    wait_for_result(&request, id, WaitLimits::for_game(running), child, progress).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::resource_files::tests::TestDir;

    #[test]
    fn responses_are_correlated_and_error_codes_keep_details_separate() {
        assert_eq!(
            response_status(&format!("{HEADER}\na\nloaded\n"), "a").unwrap(),
            LaunchOutcome::Loaded
        );
        assert_eq!(
            response_status(&format!("{HEADER}\nb\nloaded\n"), "a")
                .unwrap_err()
                .code,
            "invalid_response"
        );
        let error = response_status(
            &format!("{HEADER}\na\nerror\npackage_disabled\nraw error\n"),
            "a",
        )
        .unwrap_err();
        assert_eq!(error.code, "package_disabled");
        assert_eq!(error.detail.as_deref(), Some("raw error"));
        assert_eq!(
            response_status(&format!("{HEADER}\na\nerror\nunknown\n"), "a")
                .unwrap_err()
                .code,
            "invalid_response"
        );
        assert!(response_status("broken", "a").is_err());
    }

    #[test]
    fn unicode_requests_and_cancellation_cleanup() {
        let directory = TestDir::new();
        let (request, response) =
            write_request(&directory.0, "作者.Scene.1:/Saves/scene/中文 场景.json").unwrap();
        let text = fs::read_to_string(&request).unwrap();
        assert!(text.starts_with(HEADER));
        assert!(text.ends_with("作者.Scene.1:/Saves/scene/中文 场景.json\n"));
        assert_eq!(text.lines().count(), 4);
        assert!(!response.exists());
        fs::write(request.with_extension("accepted"), "").unwrap();
        fs::write(&response, "result").unwrap();
        drop(RequestFiles(request.clone()));
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn acceptance_changes_phase_and_loading_timeout_is_not_failure() {
        let directory = TestDir::new();
        let (request, _) = write_request(&directory.0, "Saves/scene/Test.json").unwrap();
        let id = request.file_stem().unwrap().to_str().unwrap();
        let limits = WaitLimits {
            acceptance: Duration::ZERO,
            loading: Duration::ZERO,
        };
        assert_eq!(
            wait_for_result(&request, id, limits, None, |_| {})
                .await
                .unwrap_err()
                .code,
            "accept_timeout"
        );
        fs::write(request.with_extension("accepted"), "").unwrap();
        let mut phases = Vec::new();
        assert_eq!(
            wait_for_result(&request, id, limits, None, |phase| phases
                .push(phase.to_string()))
            .await
            .unwrap(),
            LaunchOutcome::StillLoading
        );
        assert_eq!(phases, ["loading"]);
        assert_eq!(WaitLimits::for_game(false).acceptance.as_secs(), 900);
        assert_eq!(WaitLimits::for_game(true).acceptance.as_secs(), 120);
        assert_eq!(WaitLimits::for_game(true).loading.as_secs(), 600);
    }

    #[tokio::test]
    async fn waiting_refreshes_heartbeat_and_observes_delayed_completion() {
        let directory = TestDir::new();
        let (request, response) = write_request(&directory.0, "Saves/scene/Test.json").unwrap();
        let id = request.file_stem().unwrap().to_str().unwrap().to_string();
        let old = std::time::SystemTime::now() - Duration::from_secs(200);
        fs::OpenOptions::new()
            .write(true)
            .open(&request)
            .unwrap()
            .set_modified(old)
            .unwrap();
        let writer = tokio::spawn({
            let request = request.clone();
            let id = id.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(5500)).await;
                assert!(fs::metadata(&request).unwrap().modified().unwrap() > old);
                let temp = response.with_extension("result.tmp");
                fs::write(&temp, format!("{HEADER}\n{id}\nloaded\n")).unwrap();
                fs::rename(temp, response).unwrap();
            }
        });
        let limits = WaitLimits {
            acceptance: Duration::from_secs(10),
            loading: Duration::from_secs(10),
        };
        assert_eq!(
            wait_for_result(&request, &id, limits, None, |_| {})
                .await
                .unwrap(),
            LaunchOutcome::Loaded
        );
        writer.await.unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn game_exit_is_reported_promptly() {
        use std::os::windows::process::CommandExt;
        let directory = TestDir::new();
        let (request, _) = write_request(&directory.0, "Saves/scene/Test.json").unwrap();
        let child = std::process::Command::new("cmd.exe")
            .args(["/C", "exit", "1"])
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        assert_eq!(
            wait_for_result(
                &request,
                "abc",
                WaitLimits::for_game(false),
                Some(child),
                |_| {}
            )
            .await
            .unwrap_err()
            .code,
            "game_exited"
        );
    }
}
