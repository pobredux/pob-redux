//! The assistant popped out into its own window. The conversation crosses between windows through `HANDOFF`.

use std::sync::Mutex;

use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "assistant";

static HANDOFF: Mutex<Option<String>> = Mutex::new(None);

#[tauri::command]
pub fn chat_handoff_put(state: String) {
    *HANDOFF.lock().unwrap() = Some(state);
}

#[tauri::command]
pub fn chat_handoff_take() -> Option<String> {
    HANDOFF.lock().unwrap().take()
}

/// Logical pixels, as the window last stood.
#[derive(Deserialize)]
pub struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

// Async: building a window inside a synchronous command deadlocks on Windows.
// Async: building a window inside a synchronous command deadlocks on Windows.
#[tauri::command]
pub async fn assistant_window_open(app: AppHandle, bounds: Option<Bounds>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let builder = WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::default())
        .title("PoB Redux Assistant")
        .decorations(false)
        .min_inner_size(360.0, 420.0)
        .background_color(tauri::window::Color(11, 11, 12, 255));
    let builder = match bounds {
        Some(b) => builder.inner_size(b.width.max(360.0), b.height.max(420.0)).position(b.x, b.y),
        None => builder.inner_size(460.0, 820.0).center(),
    };
    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn assistant_window_close(app: AppHandle) {
    close(&app);
}

#[tauri::command]
pub fn main_window_focus(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}
