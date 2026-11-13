//! Vendors the parts of a Path of Building checkout that the headless
//! engine needs into `src-tauri/resources/<pob|pob1>`, mirroring the file set
//! of an installed PoB release (see upstream `manifest.cfg`). One checkout per
//! game: `--game poe2` (the default) takes the PathOfBuilding-PoE2 fork,
//! `--game poe1` the PathOfBuilding repository.
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
    /// Build the latest tree's sprite manifest (TreeData/<ver>/web/): decoded
    /// DDS sheets for PoE2, the shipped PNG/JPG/WebP sheets for PoE1.
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
    patches: Vec<String>,
    own_trees: Vec<String>,
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

    let tree_root = src.join("TreeData");
    let own_root = workspace.join("trees").join(game.id());
    if cli.inspect_assets {
        let (_, latest_dir) = latest_tree_version(&[&tree_root, &own_root]).context("no TreeData version")?;
        return assets::inspect(&latest_dir);
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
    let mut generated = 0usize;
    for entry in WalkDir::new(&tree_root) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(&src)?.to_path_buf();
        let name = entry.file_name().to_string_lossy();
        // Connector art (*.png) is not copied: the renderer strokes lines and arcs.
        let keep = name.ends_with(".lua") || name == "tree.json";
        if keep {
            copy_one(entry.path(), &dest.join(&rel), &mut stats)?;
            wanted.insert(rel.clone());
        }
        if name == "tree.lua" && !entry.path().with_file_name("tree.json").is_file() {
            generated += write_tree_json(entry.path(), &dest, rel.with_file_name("tree.json"), &mut stats, &mut wanted)?;
        }
    }

    // Our own exports cover a patch the pinned PoB has no tree for yet; upstream's tree wins once it ships one.
    let own_trees = own_tree_versions(&own_root, &tree_root, game)?;
    for version in &own_trees {
        let rel_dir = Path::new("TreeData").join(version);
        for entry in fs::read_dir(own_root.join(version))?.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !entry.path().is_file() || !name.ends_with(".lua") {
                continue;
            }
            let rel = rel_dir.join(&name);
            copy_one(&entry.path(), &dest.join(&rel), &mut stats)?;
            wanted.insert(rel);
            if name == "tree.lua" {
                generated += write_tree_json(&entry.path(), &dest, rel_dir.join("tree.json"), &mut stats, &mut wanted)?;
            }
        }
    }

    let latest = latest_tree_version(&[&tree_root, &own_root]);
    let web_rel = latest.as_ref().map(|(v, _)| Path::new("TreeData").join(v).join("web"));
    if cli.tree_assets {
        let (latest, src_tree) = latest.as_ref().context("no TreeData/<major>_<minor> directory found")?;
        let dest_tree = dest.join("TreeData").join(latest);
        let a = if src_tree.join("sprites.lua").is_file() {
            assets::build_sprites(src_tree, &src, &dest_tree, latest)?
        } else {
            assets::build(src_tree, &dest_tree, latest)?
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
    }
    let has_web = web_rel
        .as_ref()
        .map(|w| dest.join(w).join("manifest.json").is_file())
        .unwrap_or(false);

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
        tree_assets: has_web,
        patches: patches.clone(),
        own_trees: own_trees.clone(),
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
        // Decoded tree assets are regenerated only with --tree-assets; keep them otherwise.
        if web_rel.as_ref().map(|w| rel.starts_with(w)).unwrap_or(false) {
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
        "synced PoB {version} ({}) @ {} -> {}\n  {} files, {:.1} MB copied ({generated} tree.json generated), {} stale removed, {} patches applied, own trees: {}",
        game.id(),
        &head[..head.len().min(10)],
        dest.display(),
        stats.0,
        stats.1 as f64 / 1_048_576.0,
        removed,
        patches.len(),
        if own_trees.is_empty() { "none".to_string() } else { own_trees.join(", ") }
    );
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

/// Version folders under `trees/<game>/` that upstream's TreeData does not have.
fn own_tree_versions(own_root: &Path, upstream_tree_root: &Path, game: Game) -> Result<Vec<String>> {
    let Ok(entries) = fs::read_dir(own_root) else { return Ok(Vec::new()) };
    let mut versions = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !entry.path().join("tree.lua").is_file() {
            continue;
        }
        if upstream_tree_root.join(&name).is_dir() {
            eprintln!("warning: upstream PoB now ships TreeData/{name}; remove trees/{}/{name}", game.id());
            continue;
        }
        versions.push(name);
    }
    versions.sort();
    Ok(versions)
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

/// Variant trees ("3_29_ruthless") are not the live game's tree, and on a tie the earlier root wins.
fn latest_tree_version(tree_roots: &[&Path]) -> Option<(String, PathBuf)> {
    let mut best: Option<((u32, u32), String, PathBuf)> = None;
    for root in tree_roots {
        let Ok(entries) = fs::read_dir(root) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let mut parts = name.split('_');
            if let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) {
                if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                    if best.as_ref().map(|(k, _, _)| (a, b) > *k).unwrap_or(true) {
                        best = Some(((a, b), name.clone(), e.path()));
                    }
                }
            }
        }
    }
    best.map(|(_, n, p)| (n, p))
}
