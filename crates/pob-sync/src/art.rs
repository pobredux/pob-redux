//! Tree art by content hash: recorded in `trees/<game>/art.json`, offline copies shipped, the rest downloaded by the app.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

pub const ART_BASE: &str = "https://art.pobredux.com/tree";

#[derive(Serialize, Deserialize)]
pub struct Art {
    pub version: String,
    pub files: BTreeMap<String, ArtFile>,
    pub offline: Vec<String>,
    pub manifest: serde_json::Value,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ArtFile {
    pub hash: String,
    pub size: u64,
}

/// What the app reads from `TreeData/<version>/web/files.json`.
#[derive(Serialize)]
struct FileList<'a> {
    files: &'a BTreeMap<String, ArtFile>,
    offline: &'a [String],
}

/// The object name on R2 and in the store: `<sha256>.<extension>`.
pub fn key(rel: &str, file: &ArtFile) -> String {
    let ext = rel.rsplit_once('.').map_or("bin", |(_, e)| e);
    format!("{}.{ext}", file.hash)
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn file_entry(path: &Path) -> Result<ArtFile> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(ArtFile { hash: sha256(&bytes), size: bytes.len() as u64 })
}

/// Hashes every file of a built `web/` folder into `store` and picks the copies the app ships for offline use.
pub fn record(web: &Path, version: &str, store: &Path) -> Result<Art> {
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read(web.join("manifest.json")).context("read manifest.json")?)?;
    fs::create_dir_all(store)?;
    let mut files = BTreeMap::new();
    for entry in WalkDir::new(web) {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.file_name() == "manifest.json" {
            continue;
        }
        let rel = entry.path().strip_prefix(web)?.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(entry.path())?;
        let file = ArtFile { hash: sha256(&bytes), size: bytes.len() as u64 };
        let stored = store.join(key(&rel, &file));
        if !stored.is_file() {
            fs::write(&stored, &bytes)?;
        }
        files.insert(rel, file);
    }
    let offline = offline_copies(&manifest, version, &files);
    Ok(Art { version: version.to_string(), files, offline, manifest })
}

/// For every image the manifest draws, its quarter-size copy (or the smallest there is).
fn offline_copies(manifest: &serde_json::Value, version: &str, files: &BTreeMap<String, ArtFile>) -> Vec<String> {
    let prefix = format!("TreeData/{version}/web/");
    let lods = manifest.get("lods").and_then(|v| v.as_object());
    let mut picked = BTreeSet::new();
    for section in ["assets", "disabled"] {
        for rect in manifest.get(section).and_then(|v| v.as_object()).into_iter().flatten().map(|(_, r)| r) {
            let Some(file) = rect.get("file").and_then(|v| v.as_str()) else { continue };
            let copies: Vec<(f64, &str)> = lods
                .and_then(|l| l.get(file))
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .filter_map(|l| Some((l.get("scale")?.as_f64()?, l.get("file")?.as_str()?)))
                .collect();
            let pick = copies.iter().find(|(s, _)| *s >= 4.0).or(copies.last()).map_or(file, |(_, f)| *f);
            if let Some(rel) = pick.strip_prefix(&prefix).filter(|rel| files.contains_key(*rel)) {
                picked.insert(rel.to_string());
            }
        }
    }
    picked.into_iter().collect()
}

pub fn load(path: &Path) -> Result<Option<Art>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("read {}", path.display())),
    }
}

pub fn save(art: &Art, path: &Path) -> Result<()> {
    let mut text = serde_json::to_string_pretty(art)?;
    text.push('\n');
    if fs::read_to_string(path).ok().as_deref() != Some(text.as_str()) {
        fs::write(path, text)?;
    }
    Ok(())
}

/// Writes the manifest, the file list and the offline copies into `TreeData/<version>/web/` under `dest`.
pub fn install(art: &Art, dest: &Path, store: &Path, wanted: &mut HashSet<PathBuf>) -> Result<usize> {
    let web_rel = Path::new("TreeData").join(&art.version).join("web");
    let web = dest.join(&web_rel);
    fs::create_dir_all(&web)?;
    write_if_changed(&web.join("manifest.json"), &serde_json::to_vec(&art.manifest)?)?;
    wanted.insert(web_rel.join("manifest.json"));
    write_if_changed(&web.join("files.json"), &serde_json::to_vec(&FileList { files: &art.files, offline: &art.offline })?)?;
    wanted.insert(web_rel.join("files.json"));
    let mut fetched = 0;
    for rel in &art.offline {
        let file = art.files.get(rel).with_context(|| format!("offline copy {rel} is not in the file list"))?;
        let stored = store.join(key(rel, file));
        if !stored.is_file() {
            fetch(&key(rel, file), &file.hash, store)?;
            fetched += 1;
        }
        let out = web.join(rel);
        if fs::metadata(&out).map(|m| m.len()).ok() != Some(file.size) {
            if let Some(dir) = out.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::copy(&stored, &out)?;
        }
        wanted.insert(web_rel.join(rel));
    }
    Ok(fetched)
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<()> {
    if fs::read(path).ok().as_deref() != Some(bytes) {
        fs::write(path, bytes)?;
    }
    Ok(())
}

/// Downloads one object from R2 into the store and checks its hash.
fn fetch(key: &str, hash: &str, store: &Path) -> Result<()> {
    fs::create_dir_all(store)?;
    let part = store.join(format!("{key}.part"));
    let url = format!("{ART_BASE}/{key}");
    // Cloudflare sometimes resets an HTTP/2 stream, which curl only retries with --retry-all-errors.
    let status = Command::new("curl")
        .args(["-fsSL", "--retry", "5", "--retry-delay", "2", "--retry-all-errors", "-o"])
        .arg(&part)
        .arg(&url)
        .status()
        .context("run curl")?;
    if !status.success() {
        let _ = fs::remove_file(&part);
        bail!("download {url} failed (curl {status}): a network error, or the file is not on R2 (bun run downloads check)");
    }
    let bytes = fs::read(&part)?;
    if sha256(&bytes) != hash {
        let _ = fs::remove_file(&part);
        bail!("{url} does not match its hash");
    }
    fs::rename(&part, store.join(key))?;
    Ok(())
}
