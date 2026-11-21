//! A main-window tab torn off into its own window, labelled `tab-<view>`.

use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const PREFIX: &str = "tab-";

/// Every tab but Builds: opening a build there changes every window.
const VIEWS: [&str; 10] = ["overview", "tree", "skills", "items", "calcs", "config", "notes", "party", "optimise", "compare"];
const MIN: (f64, f64) = (480.0, 360.0);
const DEFAULT: (f64, f64) = (1100.0, 760.0);

/// Logical pixels, as the window last stood.
#[derive(Deserialize)]
pub struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// `at_cursor` puts a new window under the pointer, where a dragged-out tab was dropped.
#[tauri::command]
pub async fn tab_window_open(app: AppHandle, view: String, bounds: Option<Bounds>, at_cursor: Option<bool>) -> Result<(), String> {
    if !VIEWS.contains(&view.as_str()) {
        return Err(format!("{view} cannot open in its own window"));
    }
    let label = format!("{PREFIX}{view}");
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let (width, height) = bounds.as_ref().map_or(DEFAULT, |b| (b.width.max(MIN.0), b.height.max(MIN.1)));
    let builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::default())
        .title("PoB Redux")
        .decorations(false)
        .min_inner_size(MIN.0, MIN.1)
        .inner_size(width, height)
        .background_color(tauri::window::Color(11, 11, 12, 255));
    let cursor = if at_cursor == Some(true) { cursor_logical(&app) } else { None };
    let builder = match (cursor, bounds) {
        // The pointer lands on the new window's title bar, where the tab was held.
        (Some((x, y)), _) => builder.position(x - 80.0, y - 16.0),
        (None, Some(b)) if on_screen(&app, b.x, b.y) => builder.position(b.x, b.y),
        _ => builder.center(),
    };
    builder.build().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn tab_window_close(app: AppHandle, view: String) {
    if let Some(window) = app.get_webview_window(&format!("{PREFIX}{view}")) {
        let _ = window.destroy();
    }
}

pub fn view_of(label: &str) -> Option<&str> {
    label.strip_prefix(PREFIX)
}

pub fn close_all(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if view_of(&label).is_some() {
            let _ = window.destroy();
        }
    }
}

fn cursor_logical(app: &AppHandle) -> Option<(f64, f64)> {
    let at = app.cursor_position().ok()?;
    let scale = app.monitor_from_point(at.x, at.y).ok().flatten().map_or(1.0, |m| m.scale_factor());
    Some((at.x / scale, at.y / scale))
}

/// A monitor unplugged since the window was saved would otherwise leave it out of reach.
fn on_screen(app: &AppHandle, x: f64, y: f64) -> bool {
    let Ok(monitors) = app.available_monitors() else { return false };
    monitors.iter().any(|m| {
        let scale = m.scale_factor();
        let pos = m.position().to_logical::<f64>(scale);
        let size = m.size().to_logical::<f64>(scale);
        let (gx, gy) = (x + 80.0, y + 10.0);
        gx >= pos.x && gx < pos.x + size.width && gy >= pos.y && gy < pos.y + size.height
    })
}
