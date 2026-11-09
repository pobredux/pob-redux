//! Opening web links in the user's browser.

use tauri::{AppHandle, Url};

#[cfg(not(target_os = "linux"))]
pub fn open(app: &AppHandle, url: &str) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// The launcher, and a browser it starts, must not inherit an AppImage's library and GTK paths.
#[cfg(target_os = "linux")]
pub fn open(_app: &AppHandle, url: &str) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let mut last = None;
    for mut cmd in ::open::commands(url) {
        crate::agent::restore_host_env(&mut cmd);
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0);
        match cmd.spawn() {
            Ok(mut child) => {
                let launcher = cmd.get_program().to_string_lossy().into_owned();
                std::thread::spawn(move || match child.wait() {
                    Ok(status) if !status.success() => log::warn!("browser: {launcher} could not open the link ({status})"),
                    Err(e) => log::warn!("browser: {launcher}: {e}"),
                    Ok(_) => {}
                });
                return Ok(());
            }
            Err(e) => last = Some(e),
        }
    }
    Err(match last {
        Some(e) => format!("no program could open the link: {e}"),
        None => "no program could open the link".into(),
    })
}

#[tauri::command]
pub fn open_link(app: AppHandle, url: String) -> Result<(), String> {
    let parsed = Url::parse(&url).map_err(|e| format!("not a link: {e}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("only web links can be opened: {url}"));
    }
    open(&app, parsed.as_str())
}
