//! Assistant threads on disk: a folder per game, with a small `<id>.meta.json` for the list beside `<id>.json`.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn checked(id: &str) -> Result<&str, String> {
    if valid_id(id) {
        Ok(id)
    } else {
        Err(format!("not a thread id: {id}"))
    }
}

fn dir(app: &AppHandle, game: &str) -> Result<PathBuf, String> {
    if game != "poe1" && game != "poe2" {
        return Err(format!("unknown game {game}"));
    }
    let dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?.join("threads").join(game);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if crate::art::write_atomic(path, text.as_bytes()) {
        Ok(())
    } else {
        Err(format!("could not write {}", path.display()))
    }
}

#[tauri::command]
pub fn threads_list(app: AppHandle, game: String) -> Result<Vec<String>, String> {
    let dir = dir(&app, &game)?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        if entry.file_name().to_string_lossy().ends_with(".meta.json") {
            if let Ok(text) = std::fs::read_to_string(entry.path()) {
                out.push(text);
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn thread_load(app: AppHandle, game: String, id: String) -> Result<Option<String>, String> {
    let path = dir(&app, &game)?.join(format!("{}.json", checked(&id)?));
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// `data` is left alone when absent, so a rename or pin rewrites only the list entry.
#[tauri::command]
pub fn thread_save(app: AppHandle, game: String, id: String, meta: String, data: Option<String>) -> Result<(), String> {
    let dir = dir(&app, &game)?;
    let id = checked(&id)?;
    if let Some(data) = data {
        write(&dir.join(format!("{id}.json")), &data)?;
    }
    write(&dir.join(format!("{id}.meta.json")), &meta)
}

#[tauri::command]
pub fn thread_delete(app: AppHandle, game: String, id: String) -> Result<(), String> {
    let dir = dir(&app, &game)?;
    let id = checked(&id)?;
    for suffix in [".meta.json", ".json"] {
        let path = dir.join(format!("{id}{suffix}"));
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("could not delete {}: {e}", path.display())),
        }
    }
    if let Ok(base) = app.path().app_local_data_dir() {
        crate::agent::remove_later(base.join("agent").join(format!("t-{id}")));
    }
    Ok(())
}
