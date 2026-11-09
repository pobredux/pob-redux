//! Trade searches in an app window that shows pathofexile.com's own trade page.

use std::sync::Mutex;

use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "trade";

static REQUESTED: Mutex<Option<Url>> = Mutex::new(None);

/// pathofexile.com's login returns to the bare league page and drops the search, so reopen it once.
fn restore_search(window: &tauri::WebviewWindow, loaded: &Url) {
    if loaded.host_str() != Some("www.pathofexile.com") || !loaded.path().starts_with("/trade") {
        return;
    }
    let Some(requested) = REQUESTED.lock().unwrap().take() else { return };
    let bare = loaded.query().is_none() && loaded.path_segments().is_some_and(|s| s.filter(|s| !s.is_empty()).count() <= 4);
    if bare && *loaded != requested {
        let _ = window.navigate(requested);
    }
}

fn trade_url(url: &str) -> Result<Url, String> {
    let url = Url::parse(url).map_err(|e| e.to_string())?;
    let trade = url.scheme() == "https"
        && url.host_str() == Some("www.pathofexile.com")
        && (url.path().starts_with("/trade2/") || url.path().starts_with("/trade/"));
    if trade {
        Ok(url)
    } else {
        Err(format!("not a pathofexile.com trade search: {url}"))
    }
}

#[tauri::command]
pub async fn trade_window_open(app: AppHandle, url: String) -> Result<(), String> {
    let url = trade_url(&url)?;
    *REQUESTED.lock().unwrap() = Some(url.clone());
    if let Some(window) = app.get_webview_window(LABEL) {
        window.navigate(url).map_err(|e| e.to_string())?;
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let opener = app.clone();
    WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::External(url))
        .title("Trade")
        .inner_size(1280.0, 900.0)
        .min_inner_size(720.0, 480.0)
        .on_navigation(|url| url.scheme() == "https")
        .on_page_load(|window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                restore_search(&window, payload.url());
            }
        })
        .on_new_window(move |url, _| {
            let _ = crate::browser::open(&opener, url.as_str());
            NewWindowResponse::Deny
        })
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}
