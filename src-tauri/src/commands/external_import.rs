use crate::services::resource_files::path_key;
use std::collections::{HashSet, VecDeque};
use std::ffi::OsString;
use std::path::Path;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[derive(Default)]
pub struct ExternalImportState(Mutex<VecDeque<Vec<String>>>);

fn var_arguments(args: impl IntoIterator<Item = OsString>, cwd: &Path) -> Vec<String> {
    let mut seen = HashSet::new();
    args.into_iter()
        .skip(1) // The executable itself is the first argument in both launch paths.
        .filter_map(|arg| {
            let path = Path::new(&arg);
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("var"))
            {
                return None;
            }
            let path = if path.is_absolute() {
                path.to_path_buf()
            } else {
                cwd.join(path)
            };
            let path = path.to_string_lossy().into_owned();
            seen.insert(path_key(&path)).then_some(path)
        })
        .collect()
}

impl ExternalImportState {
    pub fn from_startup_args() -> Self {
        let state = Self::default();
        if let Ok(cwd) = std::env::current_dir() {
            let paths = var_arguments(std::env::args_os(), &cwd);
            if !paths.is_empty() {
                state.0.lock().unwrap().push_back(paths);
            }
        }
        state
    }

    fn take(&self) -> Result<Vec<Vec<String>>, String> {
        Ok(self
            .0
            .lock()
            .map_err(|e| e.to_string())?
            .drain(..)
            .collect())
    }
}

pub fn receive_launch(app: &tauri::AppHandle, args: Vec<String>, cwd: &str) {
    let paths = var_arguments(args.into_iter().map(OsString::from), Path::new(cwd));
    if paths.is_empty() {
        return;
    }
    let state = app.state::<ExternalImportState>();
    match state.0.lock() {
        Ok(mut pending) => pending.push_back(paths),
        Err(error) => {
            log::error!("Failed to queue external import: {}", error);
            return;
        }
    }
    // Events only wake the UI. The queue also survives launches before its listener is ready.
    let _ = app.emit("external-import-pending", ());
}

#[tauri::command]
pub fn take_external_import_requests(
    state: tauri::State<ExternalImportState>,
) -> Result<Vec<Vec<String>>, String> {
    state.take()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_arguments_preserve_paths_and_ignore_non_var_arguments() {
        let cwd = std::env::temp_dir();
        let absolute = cwd.join("作者 with spaces.Asset.1.VAR");
        let args = vec![
            "app.exe".into(),
            absolute.clone().into_os_string(),
            "A.Asset.2.var".into(),
            absolute.clone().into_os_string(),
            "notes.txt".into(),
            "--debug".into(),
        ];
        assert_eq!(
            var_arguments(args, &cwd),
            vec![
                absolute.to_string_lossy().into_owned(),
                cwd.join("A.Asset.2.var").to_string_lossy().into_owned(),
            ]
        );
    }

    #[test]
    fn requests_are_queued_until_consumed_once() {
        let state = ExternalImportState::default();
        state
            .0
            .lock()
            .unwrap()
            .extend([vec!["first.var".into()], vec!["second.var".into()]]);
        assert_eq!(
            state.take().unwrap(),
            vec![
                vec!["first.var".to_string()],
                vec!["second.var".to_string()]
            ]
        );
        assert!(state.take().unwrap().is_empty());
    }
}
