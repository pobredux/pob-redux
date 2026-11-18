//! An item's detail popped out into its own window, so the main window keeps its database list in view.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "item";

static CURRENT: Mutex<Option<i64>> = Mutex::new(None);

#[tauri::command]
pub fn item_window_current() -> Option<i64> {
    *CURRENT.lock().unwrap()
}

// Async: building a window inside a synchronous command deadlocks on Windows.
#[tauri::command]
pub async fn item_window_open(app: AppHandle, item_id: i64) -> Result<(), String> {
    *CURRENT.lock().unwrap() = Some(item_id);
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = app.emit_to(LABEL, "item-window:show", item_id);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::default())
        .title("PoB Redux Item")
        .decorations(false)
        .min_inner_size(380.0, 480.0)
        .inner_size(480.0, 860.0)
        .center()
        .background_color(tauri::window::Color(11, 11, 12, 255))
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn item_window_close(app: AppHandle) {
    close(&app);
}

pub fn close(app: &AppHandle) {
    *CURRENT.lock().unwrap() = None;
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}
