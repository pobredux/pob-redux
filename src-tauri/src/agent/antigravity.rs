//! Google Antigravity's ACP agent. The app downloads Google's own build, checks
//! it against a pinned hash, and runs it with a private profile; the agent signs
//! in to Google itself and keeps its tokens there.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};

use super::which::Launch;
use super::AgentStatus;

pub(crate) const VERSION: &str = "agy_acp_server_1.1.1";
const MARKER: &str = ".pob-redux-signed-in";

pub(crate) struct Asset {
    url: &'static str,
    sha256: &'static str,
    pub bytes: u64,
    exe: (&'static str, u64),
    harness: (&'static str, u64),
}

/// From the ACP registry (antigravity-acp/agent.json at 81bf71b); hashes and sizes as t3code pins them.
pub(crate) fn asset() -> Option<&'static Asset> {
    static WIN_X64: Asset = Asset {
        url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-agy_acp_server_1.1.1-windows-x86_64.zip",
        sha256: "47cb50eef14f0a4655d78cfcfda869bcea7aaee5f9787e936bc2935ea612c3b8",
        bytes: 468_238_392,
        exe: ("agy_acp_server.exe", 430_801_616),
        harness: ("localharness_external.exe", 130_971_800),
    };
    static WIN_ARM64: Asset = Asset {
        url: "https://dl.google.com/agy-extensions/releases/windows/agy-acp-server-agy_acp_server_1.1.1-windows-arm64.zip",
        sha256: "35f4b1f47ba6a3fea7b0a3e30010df5ea73a64b4f0e7cf991cddc673ddfbcafc",
        bytes: 468_521_191,
        exe: ("agy_acp_server.exe", 435_075_816),
        harness: ("localharness_external.exe", 122_455_704),
    };
    static MAC_ARM64: Asset = Asset {
        url: "https://dl.google.com/agy-extensions/releases/macos/agy-acp-server-agy_acp_server_1.1.1-darwin-arm64.zip",
        sha256: "fdfa915652cdb7ba8085cc8fffed072cbe009251aa2c951aabdda07a8c28a189",
        bytes: 316_014_828,
        exe: ("agy_acp_server.par", 802_163_856),
        harness: ("localharness_external", 116_766_704),
    };
    static LINUX_X64: Asset = Asset {
        url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-agy_acp_server_1.1.1-linux-x86_64.zip",
        sha256: "38f62d01b32deb0907b3d39a71ec301fd36369f6ffd1cf262d4af385177f79df",
        bytes: 681_969_407,
        exe: ("agy_acp_server.par", 1_880_360_328),
        harness: ("localharness_external", 128_966_920),
    };
    static LINUX_ARM64: Asset = Asset {
        url: "https://dl.google.com/agy-extensions/releases/linux/agy-acp-server-agy_acp_server_1.1.1-linux-arm64.zip",
        sha256: "ed69e64b308fcb123ab54bf3277bf9cb0d651064f885ea5aab0ff520c7175398",
        bytes: 656_572_786,
        exe: ("agy_acp_server.par", 1_862_073_131),
        harness: ("localharness_external", 122_158_704),
    };
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some(&WIN_X64),
        ("windows", "aarch64") => Some(&WIN_ARM64),
        ("macos", "aarch64") => Some(&MAC_ARM64),
        ("linux", "x86_64") => Some(&LINUX_X64),
        ("linux", "aarch64") => Some(&LINUX_ARM64),
        _ => None,
    }
}

/// Credentials and settings from the user's own Google and Gemini setup stay out of the private profile.
const REMOVED: &[&str] = &[
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "GOOGLE_CLOUD_QUOTA_PROJECT",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GCLOUD_PROJECT",
    "CLOUDSDK_CORE_PROJECT",
    "AGY_ACP_CCPA_PROJECT",
    "AGY_ACP_ENABLE_OAUTH",
    "GEMINI_HOME",
    "AGY_ACP_FORCE_FILE_STORAGE",
    "ANTIGRAVITY_HARNESS_PATH",
    "BROWSER",
    "PYTHONUNBUFFERED",
];

fn local(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_local_data_dir().map_err(|e| e.to_string())
}

fn install_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(local(app)?.join("tools").join("antigravity").join(VERSION))
}

pub(crate) fn profile(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(local(app)?.join("antigravity-profile"))
}

fn managed(app: &AppHandle) -> Option<PathBuf> {
    let a = asset()?;
    let dir = install_dir(app).ok()?;
    let ok = |name: &str, size: u64| std::fs::metadata(dir.join(name)).is_ok_and(|m| m.len() == size);
    (ok(a.exe.0, a.exe.1) && ok(a.harness.0, a.harness.1)).then(|| dir.join(a.exe.0))
}

fn exe_path(app: &AppHandle, configured: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = configured.map(str::trim).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p)).filter(|p| p.is_file());
    }
    managed(app).or_else(|| super::which::find("agy_acp_server", None).map(|l| l.program))
}

fn harness_of(exe: &Path) -> PathBuf {
    let name = if cfg!(windows) { "localharness_external.exe" } else { "localharness_external" };
    exe.with_file_name(name)
}

/// The agent unpacks about 1 GB per launch; it goes under the app's own
/// folder, which is cleared at startup, and stays short for Windows' path limit.
pub(crate) fn launch(app: &AppHandle, configured: Option<&str>) -> Option<Launch> {
    let exe = exe_path(app, configured)?;
    let profile = profile(app).ok()?;
    let settings = profile.join("antigravity-acp");
    std::fs::create_dir_all(&settings).ok()?;
    std::fs::write(settings.join("settings.json"), "{\"auth\":{\"type\":\"oauth-personal\"}}\n").ok()?;
    let temp = local(app).ok()?.join("agent").join("t").join(super::random_hex(4).ok()?);
    std::fs::create_dir_all(&temp).ok()?;

    let mut env: Vec<(String, OsString)> = vec![
        ("GEMINI_HOME".into(), profile.into_os_string()),
        ("AGY_ACP_FORCE_FILE_STORAGE".into(), "1".into()),
        ("ANTIGRAVITY_HARNESS_PATH".into(), harness_of(&exe).into_os_string()),
        ("PYTHONUNBUFFERED".into(), "1".into()),
    ];
    if cfg!(windows) {
        env.push(("TEMP".into(), temp.clone().into_os_string()));
        env.push(("TMP".into(), temp.clone().into_os_string()));
    } else {
        env.push(("TMPDIR".into(), temp.clone().into_os_string()));
    }
    let prefix = if cfg!(target_os = "linux") { vec!["--uid=".into()] } else { Vec::new() };
    Some(Launch { program: exe, prefix, env, remove: REMOVED.to_vec(), scratch: Some(temp) })
}

/// Starting the agent unpacks about 1 GB, so the status comes from the files, not a probe.
pub(crate) fn status(app: &AppHandle, configured: Option<&str>) -> AgentStatus {
    let Some(exe) = exe_path(app, configured) else {
        return AgentStatus::default();
    };
    let signed_in = profile(app).is_ok_and(|p| p.join(MARKER).is_file());
    AgentStatus {
        installed: true,
        path: Some(exe.display().to_string()),
        version: Some(VERSION.trim_start_matches("agy_acp_server_").to_string()),
        signed_in: Some(signed_in),
        account: signed_in.then(|| "Google account".to_string()),
        error: None,
    }
}

pub(crate) fn mark_signed_in(app: &AppHandle, yes: bool) {
    let Ok(p) = profile(app) else { return };
    if yes {
        let _ = std::fs::create_dir_all(&p);
        let _ = std::fs::write(p.join(MARKER), "");
    } else {
        let _ = std::fs::remove_file(p.join(MARKER));
    }
}

pub(crate) fn uninstall(app: &AppHandle) -> Result<(), String> {
    let dir = install_dir(app)?;
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

/// Signing out removes the private profile, which holds the agent's Google tokens.
pub(crate) fn sign_out(app: &AppHandle) -> Result<(), String> {
    let p = profile(app)?;
    match std::fs::remove_dir_all(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Progress {
    Download { done: u64, total: u64 },
    Unpack,
}

fn extract(zip_path: &Path, dir: &Path, a: &Asset) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("the download is not a valid archive: {e}"))?;
    let expected = [a.exe, a.harness];
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    if names.len() != expected.len() || !expected.iter().all(|(n, _)| names.iter().any(|x| x == n)) {
        return Err(format!("the archive holds unexpected files: {}", names.join(", ")));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    for (name, size) in expected {
        let mut entry = zip.by_name(name).map_err(|e| e.to_string())?;
        if entry.size() != size {
            return Err(format!("{name} has the wrong size in the archive"));
        }
        let out_path = dir.join(name);
        let mut out = std::fs::File::create(&out_path).map_err(|e| format!("{}: {e}", out_path.display()))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("{}: {e}", out_path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out_path, std::fs::Permissions::from_mode(0o755));
        }
    }
    Ok(())
}

/// Download Google's build, check its hash and contents, and unpack it.
pub(crate) async fn install(app: &AppHandle, on_progress: &Channel<Progress>) -> Result<(), String> {
    let a = asset().ok_or("Antigravity has no build for this computer")?;
    let dir = install_dir(app)?;
    let parent = dir.parent().ok_or("no install folder")?.to_path_buf();
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let part = parent.join(format!("{VERSION}.zip.part"));

    let client = reqwest::Client::builder().build().map_err(|e| e.to_string())?;
    let mut res = client.get(a.url).send().await.map_err(|e| format!("could not reach Google's download server: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("Google's download server returned {}", res.status()));
    }
    let mut file = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
    let mut hash = Sha256::new();
    let mut done: u64 = 0;
    let mut reported = 0;
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("the download stopped: {e}"))? {
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|e| format!("{}: {e}", part.display()))?;
        done += chunk.len() as u64;
        let pct = done * 100 / a.bytes.max(1);
        if pct != reported {
            reported = pct;
            let _ = on_progress.send(Progress::Download { done, total: a.bytes });
        }
    }
    drop(file);
    let digest: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if done != a.bytes || digest != a.sha256 {
        let _ = std::fs::remove_file(&part);
        return Err("The download does not match Google's published build, so it was deleted. Try again later.".into());
    }
    let _ = on_progress.send(Progress::Unpack);
    let (zip_path, target) = (part.clone(), dir.clone());
    let unpacked = tokio::task::spawn_blocking(move || extract(&zip_path, &target, a)).await.map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&part);
    if let Err(e) = unpacked {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(e);
    }
    Ok(())
}
