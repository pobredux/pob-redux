//! Vendors the parts of a Path of Building checkout that the headless
//! engine needs into `src-tauri/resources/<pob|pob1>`, mirroring the file set
//! of an installed PoB release (see upstream `manifest.cfg`). One checkout per
//! game: `--game poe2` (the default) takes the PathOfBuilding-PoE2 fork,
//! `--game poe1` the PathOfBuilding repository. Passive trees and their art
//! come from this workspace's `trees/<game>/`, never from the checkout.
//!
//!   cargo run -p pob-sync -- [--game poe1|poe2] [--source <checkout>] [--dest <dir>] [--clean] [--tree-assets]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

mod art;
mod assets;
mod lua_json;

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
enum Game {
    Poe1,
    Poe2,
}

impl Game {
    fn id(self) -> &'static str {
        match self {
            Game::Poe1 => "poe1",
            Game::Poe2 => "poe2",
        }
    }
    /// Resource directory under `src-tauri/resources`; PoE2 keeps the original name.
    fn dest_name(self) -> &'static str {
        match self {
            Game::Poe1 => "pob1",
            Game::Poe2 => "pob",
        }
    }
    fn repo_name(self) -> &'static str {
        match self {
            Game::Poe1 => "PathOfBuilding",
            Game::Poe2 => "PathOfBuilding-PoE2",
        }
    }
}

#[derive(Parser)]
#[command(name = "pob-sync", about = "Vendor Path of Building (PoE1 or PoE2) into app resources")]
struct Cli {
    /// Which game's Path of Building to vendor.
    #[arg(long, value_enum, default_value = "poe2")]
    game: Game,
    /// Checkout root. Defaults to the game's `source_path` in pob-sync.toml.
    #[arg(long, env = "POB_SOURCE")]
    source: Option<PathBuf>,
    /// Output directory. Defaults to <workspace>/src-tauri/resources/pob (PoE2) or pob1 (PoE1).
    #[arg(long)]
    dest: Option<PathBuf>,
    /// Delete the destination first.
    #[arg(long)]
    clean: bool,
    /// Rebuild the newest tree's art into target/tree-art and record it in trees/<game>/art.json.
    #[arg(long)]
    tree_assets: bool,
    /// Proceed even if the checkout's HEAD differs from the pinned commit.
    #[arg(long)]
    allow_commit_mismatch: bool,
    /// Print DDS header details for the latest tree's sheets and exit.
    #[arg(long)]
    inspect_assets: bool,
}

#[derive(Deserialize, Default, Clone)]
struct GameToml {
    source_path: Option<String>,
    commit: Option<String>,
}

/// `[poe1]` and `[poe2]` tables; bare `source_path`/`commit` keys are the PoE2 profile.
#[derive(Deserialize, Default)]
struct SyncToml {
    source_path: Option<String>,
    commit: Option<String>,
    poe1: Option<GameToml>,
    poe2: Option<GameToml>,
}

impl SyncToml {
    fn profile(&self, game: Game) -> GameToml {
        match game {
            Game::Poe1 => self.poe1.clone().unwrap_or_default(),
            Game::Poe2 => self
                .poe2
                .clone()
                .unwrap_or_else(|| GameToml { source_path: self.source_path.clone(), commit: self.commit.clone() }),
        }
    }
}

#[derive(Serialize)]
struct SyncInfo {
    game: String,
    upstream_commit: String,
    upstream_commit_date: String,
    upstream_version: String,
    pinned_commit: Option<String>,
    synced_at_unix: u64,
    files: usize,
    bytes: u64,
    tree_assets: bool,
    tree_art: Option<String>,
    patches: Vec<String>,
    trees: Vec<String>,
    unlisted_trees: Vec<String>,
}

const EXCLUDE_FILES: &[&str] = &[
    "HeadlessWrapper.lua",
    "LaunchInstall.lua",
    "Settings.xml",
    "_SimpleGraphic.def.lua",
    "luacov.stats.out",
    "poe_api_response.json",
];
const EXCLUDE_DIRS: &[&str] = &["Export", "TreeData", "Builds"];
const ROOT_FILES: &[&str] = &["changelog.txt", "LICENSE.md", "help.txt"];

fn main() -> Result<()> {
    let cli = Cli::parse();
    let game = cli.game;
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .context("locate workspace root")?;

    let toml_cfg: SyncToml = fs::read_to_string(workspace.join("pob-sync.toml"))
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default();
    let profile = toml_cfg.profile(game);

    let source = match cli.source {
        Some(s) => s,
        None => {
            let rel = profile
                .source_path
                .as_deref()
                .with_context(|| format!("no --source given and pob-sync.toml has no source_path for {}", game.id()))?;
            workspace.join(rel)
        }
    };
    let source = source
        .canonicalize()
        .with_context(|| format!("source {} not found", source.display()))?;
    let src = source.join("src");
    if !src.join("Launch.lua").is_file() {
        bail!("{} does not look like a {} checkout (no src/Launch.lua)", source.display(), game.repo_name());
    }
    let dest = cli
        .dest
        .unwrap_or_else(|| workspace.join("src-tauri/resources").join(game.dest_name()));

    let tree_root = workspace.join("trees").join(game.id());
    let pob_tree_root = src.join("TreeData");
    if cli.inspect_assets {
        let (latest, latest_dir) = latest_tree_version(&tree_root).context("no tree version in trees/")?;
        return assets::inspect(&sheet_dir(&latest_dir, &pob_tree_root.join(latest)));
    }

    let head = git(&source, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".into());
    let head_date = git(&source, &["log", "-1", "--format=%cI"]).unwrap_or_default();
    if let Some(pinned) = &profile.commit {
        if pinned != &head {
            let msg = format!("checkout HEAD {head} differs from pinned commit {pinned}");
            if cli.allow_commit_mismatch {
                eprintln!("warning: {msg}");
            } else {
                bail!("{msg} (pass --allow-commit-mismatch, or update pob-sync.toml)");
            }
        }
    }
    let version = read_version(&source.join("manifest.xml")).unwrap_or_else(|| "unknown".into());

    if cli.clean && dest.exists() {
        fs::remove_dir_all(&dest).context("clean destination")?;
    }
    fs::create_dir_all(&dest)?;

    let mut wanted: HashSet<PathBuf> = HashSet::new();
    let mut stats = (0usize, 0u64);

    // Program files (src/ minus exclusions). PoE1's timeless jewel archives
    // (170 MB of .zip.part files) ship as upstream does: PoB inflates them on
    // first use and caches the result next to them, which the host redirects
    // to the user's cache directory.
    for entry in WalkDir::new(&src).into_iter().filter_entry(|e| {
        if e.depth() == 1 && e.file_type().is_dir() {
            let name = e.file_name().to_string_lossy();
            return !EXCLUDE_DIRS.contains(&name.as_ref());
        }
        true
    }) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if EXCLUDE_FILES.contains(&name.as_ref()) || name.ends_with(".bin") {
            continue;
        }
        let rel = entry.path().strip_prefix(&src)?.to_path_buf();
        copy_one(entry.path(), &dest.join(&rel), &mut stats)?;
        wanted.insert(rel);
    }

    // Tree data: Lua tables for the engine, JSON for the renderer. PoE1 ships
    // no tree.json, so one is generated from tree.lua per version.
    let trees = tree_versions(&tree_root)?;
    let listed = listed_tree_versions(&src.join("GameVersions.lua"))?;
    let missing: Vec<&str> = listed.iter().filter(|v| !trees.contains(v)).map(String::as_str).collect();
    if !missing.is_empty() {
        bail!("PoB lists tree versions that trees/{} lacks: {} (export them with ggpk-explorer export-tree)", game.id(), missing.join(", "));
    }
    let unlisted: Vec<String> = trees.iter().filter(|v| !listed.contains(v)).cloned().collect();
    let mut generated = 0usize;
    for version in &trees {
        let rel_dir = Path::new("TreeData").join(version);
        // Connector art (*.png) is not copied: the renderer strokes lines and arcs.
        for entry in fs::read_dir(tree_root.join(version))?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.path().is_file() && (name.ends_with(".lua") || name == "tree.json") {
                let rel = rel_dir.join(&name);
                copy_one(&entry.path(), &dest.join(&rel), &mut stats)?;
                wanted.insert(rel);
            }
        }
        if !tree_root.join(version).join("tree.json").is_file() {
            generated += write_tree_json(&tree_root.join(version).join("tree.lua"), &dest, rel_dir.join("tree.json"), &mut stats, &mut wanted)?;
        }
    }
    // PoE1's PassiveTree.lua requires TreeData/legion/tree-legion.lua for every tree.
    for entry in fs::read_dir(tree_root.join("legion")).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".lua") {
            let rel = Path::new("TreeData").join("legion").join(&name);
            copy_one(&entry.path(), &dest.join(&rel), &mut stats)?;
            wanted.insert(rel);
        }
    }
    let versions_rel = Path::new("TreeData").join("versions.lua");
    let versions_lua = format!("return {{ {} }}\n", trees.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(", "));
    if fs::read_to_string(dest.join(&versions_rel)).ok().as_deref() != Some(versions_lua.as_str()) {
        fs::create_dir_all(dest.join("TreeData"))?;
        fs::write(dest.join(&versions_rel), &versions_lua)?;
    }
    wanted.insert(versions_rel);

    let (latest, src_tree) = latest_tree_version(&tree_root).context("no <major>_<minor> tree folder in trees/")?;
    let art_path = tree_root.join("art.json");
    let store = workspace.join("target").join("tree-art").join("store");
    if cli.tree_assets {
        let build_tree = workspace.join("target").join("tree-art").join("build").join(game.id()).join(&latest);
        let a = if src_tree.join("sprites.lua").is_file() {
            assets::build_sprites(&src_tree, &tree_root, &build_tree, &latest)?
        } else {
            let sheets = sheet_dir(&src_tree, &pob_tree_root.join(&latest));
            println!("tree sheets: {}", sheets.display());
            assets::build(&src_tree, &sheets, &tree_root, &build_tree, &latest)?
        };
        println!(
            "tree assets: {} sheets, {} layers -> {} files, {:.1} MB",
            a.sheets,
            a.layers,
            a.files,
            a.bytes as f64 / 1_048_576.0
        );
        for s in &a.skipped {
            eprintln!("  skipped {s}");
        }
        art::save(&art::record(&build_tree.join("web"), &latest, &store)?, &art_path)?;
    }
    let tree_art = art::load(&art_path)?;
    let mut fetched = 0;
    if let Some(a) = &tree_art {
        if a.version != latest {
            bail!("trees/{}/art.json is for tree {} but the newest tree is {latest}; rebuild it with --tree-assets", game.id(), a.version);
        }
        fetched = art::install(a, &dest, &store, &mut wanted)?;
    }

    // Pure-Lua libraries from the runtime (json, xml, base64, sha1, ...).
    let rt_lua = source.join("runtime/lua");
    for entry in WalkDir::new(&rt_lua) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.file_name().to_string_lossy() == "lua-profiler.lua" {
            continue;
        }
        let rel = Path::new("lua").join(entry.path().strip_prefix(&rt_lua)?);
        copy_one(entry.path(), &dest.join(&rel), &mut stats)?;
        wanted.insert(rel);
    }

    for f in ROOT_FILES {
        let p = source.join(f);
        if p.is_file() {
            copy_one(&p, &dest.join(f), &mut stats)?;
            wanted.insert(PathBuf::from(f));
        }
    }

    let patches = apply_patches(&workspace.join("patches").join(game.id()), &src, &dest, &mut wanted)?;

    // A release-style manifest: Launch.lua treats a manifest without branch and
    // platform as a dev checkout and changes user-path and update behaviour.
    let manifest = format!(
        "<?xml version='1.0' encoding='UTF-8'?>\n<PoBVersion>\n\t<Version number=\"{version}\" branch=\"release\" platform=\"win32\" />\n</PoBVersion>\n"
    );
    fs::write(dest.join("manifest.xml"), manifest)?;
    wanted.insert(PathBuf::from("manifest.xml"));

    let info = SyncInfo {
        game: game.id().to_string(),
        upstream_commit: head.clone(),
        upstream_commit_date: head_date,
        upstream_version: version.clone(),
        pinned_commit: profile.commit.clone(),
        synced_at_unix: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        files: stats.0,
        bytes: stats.1,
        tree_assets: tree_art.is_some(),
        tree_art: tree_art.as_ref().map(|a| a.version.clone()),
        patches: patches.clone(),
        trees: trees.clone(),
        unlisted_trees: unlisted.clone(),
    };
    fs::write(dest.join("SYNC.json"), serde_json::to_string_pretty(&info)?)?;
    wanted.insert(PathBuf::from("SYNC.json"));

    // Remove stale files from previous syncs.
    let mut removed = 0;
    for entry in WalkDir::new(&dest).contents_first(true) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(&dest)?.to_path_buf();
        if rel.as_os_str().is_empty() {
            continue;
        }
        if entry.file_type().is_file() && !wanted.contains(&rel) {
            fs::remove_file(entry.path())?;
            removed += 1;
        } else if entry.file_type().is_dir() && fs::read_dir(entry.path())?.next().is_none() {
            let _ = fs::remove_dir(entry.path());
        }
    }

    println!(
        "synced PoB {version} ({}) @ {} -> {}\n  {} files, {:.1} MB copied ({generated} tree.json generated), {} stale removed, {} patches applied, {} trees, not in PoB's list: {}",
        game.id(),
        &head[..head.len().min(10)],
        dest.display(),
        stats.0,
        stats.1 as f64 / 1_048_576.0,
        removed,
        patches.len(),
        trees.len(),
        if unlisted.is_empty() { "none".to_string() } else { unlisted.join(", ") }
    );
    match &tree_art {
        Some(a) => println!("  tree art {}: {} files, {} shipped for offline use, {fetched} fetched from R2", a.version, a.files.len(), a.offline.len()),
        None => println!("  no trees/{}/art.json: the app has no tree art (build it with --tree-assets)", game.id()),
    }
    Ok(())
}

/// Returns 1 when tree.json was regenerated.
fn write_tree_json(tree_lua: &Path, dest: &Path, out_rel: PathBuf, stats: &mut (usize, u64), wanted: &mut HashSet<PathBuf>) -> Result<usize> {
    let out = dest.join(&out_rel);
    wanted.insert(out_rel);
    if up_to_date(tree_lua, &out) {
        return Ok(0);
    }
    let json = lua_json::eval_file(tree_lua)?;
    fs::write(&out, serde_json::to_vec(&json)?)?;
    stats.0 += 1;
    stats.1 += fs::metadata(&out)?.len();
    Ok(1)
}

/// Version folders ("3_29", "3_29_ruthless") under `trees/<game>/` that hold a tree.lua.
fn tree_versions(root: &Path) -> Result<Vec<String>> {
    let mut versions = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("read {}", root.display()))?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if version_key(&name).is_some() && entry.path().join("tree.lua").is_file() {
            versions.push(name);
        }
    }
    versions.sort();
    Ok(versions)
}

/// The `treeVersionList` entries in PoB's GameVersions.lua.
fn listed_tree_versions(game_versions: &Path) -> Result<Vec<String>> {
    let text = fs::read_to_string(game_versions).with_context(|| format!("read {}", game_versions.display()))?;
    let list = text.split_once("treeVersionList").context("GameVersions.lua has no treeVersionList")?.1;
    let (_, body) = list.split_once('{').context("treeVersionList is not a table")?;
    let (body, _) = body.split_once('}').context("treeVersionList is not closed")?;
    Ok(body.split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect())
}

/// Where the newest PoE2 tree's DDS sheets are; they stay in the PoB checkout until tree art is served from R2.
fn sheet_dir(own: &Path, pob: &Path) -> PathBuf {
    let has_sheets = fs::read_dir(own).into_iter().flatten().flatten().any(|e| e.file_name().to_string_lossy().ends_with(".dds.zst"));
    if has_sheets { own.to_path_buf() } else { pob.to_path_buf() }
}

/// Fixes carried in `patches/<game>/` until upstream merges them. Every file a
/// patch touches is recopied from the source once, before any patch applies, so
/// a re-run applies cleanly and later patches can build on earlier ones.
fn apply_patches(dir: &Path, src: &Path, dest: &Path, wanted: &mut HashSet<PathBuf>) -> Result<Vec<String>> {
    let Ok(entries) = fs::read_dir(dir) else { return Ok(Vec::new()) };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".patch"))
        .collect();
    names.sort();
    let patches: Vec<String> = names.iter().map(|n| plain_path(&dir.join(n)).to_string_lossy().to_string()).collect();
    let mut recopied = HashSet::new();
    for (name, patch) in names.iter().zip(&patches) {
        let touched = git_apply(dest, &["--numstat", patch]).with_context(|| format!("read {name}"))?;
        for line in touched.lines() {
            let Some(rel) = line.split('\t').nth(2) else { continue };
            let rel = PathBuf::from(rel);
            wanted.insert(rel.clone());
            if !recopied.insert(rel.clone()) {
                continue;
            }
            let from = src.join(&rel);
            if from.is_file() {
                fs::copy(&from, dest.join(&rel)).with_context(|| format!("recopy {}", rel.display()))?;
            }
        }
    }
    for (name, patch) in names.iter().zip(&patches) {
        git_apply(dest, &[patch]).with_context(|| format!("apply {name}"))?;
    }
    Ok(names)
}

/// `-p2` drops upstream's `a/src/` so paths land at the destination root; the
/// ceiling stops git from finding this workspace's repository and resolving them there.
fn git_apply(dest: &Path, args: &[&str]) -> Result<String> {
    let dest = plain_path(dest);
    let ceiling = dest.parent().context("patch destination has no parent")?;
    let out = Command::new("git")
        .current_dir(&dest)
        .env("GIT_CEILING_DIRECTORIES", ceiling)
        .args(["apply", "-p2"])
        .args(args)
        .output()?;
    if !out.status.success() {
        bail!("git apply {:?} failed:\n{}", args, String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Windows canonical paths carry a `\\?\` prefix that git does not accept.
fn plain_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s))
}

/// True when `to` exists, is not empty, and is not older than `from`.
fn up_to_date(from: &Path, to: &Path) -> bool {
    let (Ok(src_md), Ok(dst_md)) = (fs::metadata(from), fs::metadata(to)) else { return false };
    if dst_md.len() == 0 {
        return false;
    }
    match (src_md.modified(), dst_md.modified()) {
        (Ok(s), Ok(d)) => s <= d,
        _ => false,
    }
}

fn copy_one(from: &Path, to: &Path, stats: &mut (usize, u64)) -> Result<()> {
    let src_md = fs::metadata(from)?;
    if let Ok(dst_md) = fs::metadata(to) {
        let same_len = dst_md.len() == src_md.len();
        let newer = match (src_md.modified(), dst_md.modified()) {
            (Ok(s), Ok(d)) => s > d,
            _ => true,
        };
        if same_len && !newer {
            return Ok(());
        }
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(from, to).with_context(|| format!("copy {}", from.display()))?;
    stats.0 += 1;
    stats.1 += src_md.len();
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output()?;
    if !out.status.success() {
        bail!("git {:?} failed", args);
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn read_version(manifest: &Path) -> Option<String> {
    let text = fs::read_to_string(manifest).ok()?;
    let i = text.find("<Version number=\"")?;
    let rest = &text[i + "<Version number=\"".len()..];
    let j = rest.find('"')?;
    Some(rest[..j].to_string())
}

/// (major, minor, is a variant such as "_ruthless") for a tree folder name.
fn version_key(name: &str) -> Option<(u32, u32, bool)> {
    let mut parts = name.splitn(3, '_');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor, parts.next().is_some()))
}

/// Variant trees ("3_29_ruthless") are not the live game's tree.
fn latest_tree_version(root: &Path) -> Option<(String, PathBuf)> {
    let mut best: Option<((u32, u32), String, PathBuf)> = None;
    for e in fs::read_dir(root).ok()?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((major, minor, false)) = version_key(&name) else { continue };
        if best.as_ref().map(|(k, _, _)| (major, minor) > *k).unwrap_or(true) {
            best = Some(((major, minor), name, e.path()));
        }
    }
    best.map(|(_, n, p)| (n, p))
}
