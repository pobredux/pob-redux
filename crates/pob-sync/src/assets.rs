//! Decodes PoB's passive-tree sprite sheets into WebP for the web renderer.
//!
//! Each `<name>_<w>_<h>_<format>.dds.zst` is a zstd-compressed DDS array
//! texture: one named asset per layer, all layers the same size. tree.json's
//! `ddsCoords[file][name]` gives the 1-based layer of every asset, or
//! `[x, y, w, h, layer]` when the art does not fill its layer. Small
//! layers are packed into one atlas per sheet; large ones (backgrounds) get a
//! file per layer, downscaled. `web/manifest.json` maps asset names to rects.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use image::RgbaImage;
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct AssetRect {
    pub file: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Native layer size before any downscaling.
    pub ow: u32,
    pub oh: u32,
    /// Masked to the ellipse inside the rect, so the renderer can skip its clip.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub round: bool,
}

#[derive(Serialize, Default)]
pub struct Manifest {
    pub version: String,
    pub assets: BTreeMap<String, AssetRect>,
    /// Greyscale icon variants from the `skills-disabled_*` sheets.
    pub disabled: BTreeMap<String, AssetRect>,
    /// Smaller copies of a sheet, keyed by its file; a rect maps into one divided by `scale`.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub lods: BTreeMap<String, Vec<Lod>>,
}

#[derive(Serialize, Clone)]
pub struct Lod {
    pub file: String,
    pub scale: u32,
}

#[derive(Default)]
pub struct AssetStats {
    pub sheets: usize,
    pub layers: usize,
    pub files: usize,
    pub bytes: u64,
    pub skipped: Vec<String>,
}

pub fn build(src_tree: &Path, sheet_dir: &Path, art_root: &Path, dest_tree: &Path, version: &str) -> Result<AssetStats> {
    let tree: serde_json::Value =
        serde_json::from_slice(&fs::read(src_tree.join("tree.json")).context("read tree.json")?)?;
    let web = dest_tree.join("web");
    if web.exists() {
        fs::remove_dir_all(&web)?;
    }
    fs::create_dir_all(&web)?;

    let mut manifest = Manifest { version: version.to_string(), ..Default::default() };
    let mut stats = AssetStats::default();

    if let Some(coords) = tree.get("ddsCoords").and_then(|v| v.as_object()) {
        // PoE2's skills-disabled sheets are byte-identical to skills; a repeat reuses the first image.
        let mut sheets: Vec<_> = coords.iter().collect();
        sheets.sort_by_key(|(file, _)| (file.starts_with("skills-disabled"), file.as_str()));
        let mut written: Vec<(Vec<u8>, String)> = Vec::new();
        for (file, names) in sheets {
            let path = sheet_dir.join(file);
            if !path.is_file() {
                stats.skipped.push(format!("{file}: missing"));
                continue;
            }
            let raw = fs::read(&path)?;
            let same = written.iter().find(|(bytes, _)| *bytes == raw).map(|(_, base)| base.clone());
            let decoded = decode_sheet(&raw, file, names, version, &web, same.as_deref());
            if same.is_none() {
                written.push((raw, file.strip_suffix(".dds.zst").unwrap_or(file).to_string()));
            }
            match decoded {
                Ok(out) => {
                    stats.sheets += 1;
                    stats.layers += out.layers;
                    stats.files += out.files;
                    stats.bytes += out.bytes;
                    let target = if out.disabled { &mut manifest.disabled } else { &mut manifest.assets };
                    target.extend(out.rects);
                    manifest.lods.extend(out.lods);
                }
                Err(e) => stats.skipped.push(format!("{file}: {e:#}")),
            }
        }
    }

    // tree.json "assets" (connector line/arc PNGs) are intentionally not
    // shipped; the renderer strokes connectors itself.

    copy_standalone(art_root, &web, version, &ring_entries(), &mut manifest, &mut stats)?;
    fs::write(web.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(stats)
}

/// Prints the DDS header of every sheet next to the data actually present.
pub fn inspect(src_tree: &Path) -> Result<()> {
    let mut files: Vec<_> = fs::read_dir(src_tree)?
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".dds.zst"))
        .collect();
    files.sort_by_key(|e| e.file_name());
    for e in files {
        let raw = zstd::decode_all(&fs::read(e.path())?[..])?;
        let dds = image_dds::ddsfile::Dds::read(&raw[..])?;
        let (w, h) = (dds.get_width(), dds.get_height());
        let layers = dds.get_num_array_layers();
        let mips = dds.get_num_mipmap_levels();
        let fmt = dds
            .get_dxgi_format()
            .map(|f| format!("{f:?}"))
            .or_else(|| dds.get_d3d_format().map(|f| format!("{f:?}")))
            .unwrap_or_else(|| "?".into());
        let data = dds.data.len();
        println!(
            "{:<48} {:>5}x{:<5} layers={:<4} mips={:<3} fmt={:<14} data={:>10} bytes  per-layer={}",
            e.file_name().to_string_lossy(),
            w,
            h,
            layers,
            mips,
            fmt,
            data,
            if layers > 0 { data / layers as usize } else { 0 }
        );
    }
    Ok(())
}

/// Icons stay lossless (`None`); large soft art (class plates, mastery effects) is lossy.
fn save_webp(img: &RgbaImage, out: &Path, quality: Option<f32>) -> Result<()> {
    let enc = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height());
    let mem = match quality {
        None => enc.encode_lossless(),
        Some(q) => enc.encode(q),
    };
    fs::write(out, &*mem).with_context(|| format!("write {}", out.display()))?;
    Ok(())
}

/// Clears outside the rect's inscribed ellipse, the circle the renderer clips icons to.
fn mask_round(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32) {
    let (rx, ry) = (w as f32 / 2.0, h as f32 / 2.0);
    for py in y..(y + h).min(img.height()) {
        for px in x..(x + w).min(img.width()) {
            let dx = ((px - x) as f32 + 0.5 - rx) / rx;
            let dy = ((py - y) as f32 + 0.5 - ry) / ry;
            let inside = (1.0 - (dx * dx + dy * dy).sqrt()) * rx.min(ry);
            let cover = (inside + 0.5).clamp(0.0, 1.0);
            let p = img.get_pixel_mut(px, py);
            p[3] = (p[3] as f32 * cover).round() as u8;
            if p[3] == 0 {
                p.0 = [0, 0, 0, 0];
            }
        }
    }
}

/// Node icon sheets: every rect in them is drawn inside a circle.
fn is_icon_sheet(base: &str) -> bool {
    base.starts_with("skills") || (base.starts_with("mastery") && !base.starts_with("mastery-active-effect"))
}

/// Sheet file names differ between GGG's export and ours, so PoE1 finds its node icons by `sprites.lua` section.
fn is_icon_section(section: &str) -> bool {
    matches!(
        section,
        "normalActive"
            | "notableActive"
            | "keystoneActive"
            | "normalInactive"
            | "notableInactive"
            | "keystoneInactive"
            | "mastery"
            | "masteryConnected"
            | "masteryActiveSelected"
            | "masteryInactive"
    )
}

/// 1/`f` size by block average, alpha-weighted so masked-out pixels do not darken edges.
fn shrink(img: &RgbaImage, f: u32) -> RgbaImage {
    let (w, h) = (img.width().div_ceil(f), img.height().div_ceil(f));
    let block = (f * f) as f32;
    RgbaImage::from_fn(w, h, |ox, oy| {
        let mut acc = [0f32; 4];
        for y in oy * f..((oy + 1) * f).min(img.height()) {
            for x in ox * f..((ox + 1) * f).min(img.width()) {
                let p = img.get_pixel(x, y);
                let a = p[3] as f32;
                for c in 0..3 {
                    acc[c] += p[c] as f32 * a;
                }
                acc[3] += a;
            }
        }
        if acc[3] == 0.0 {
            return image::Rgba([0, 0, 0, 0]);
        }
        let c = |i: usize| (acc[i] / acc[3]).round() as u8;
        image::Rgba([c(0), c(1), c(2), (acc[3] / block).round() as u8])
    })
}

/// Halved copies of `img` at `web/lod<scale>/<rel>` until its largest sprite would drop under `min` px.
fn write_lods(img: &RgbaImage, largest: u32, min: u32, web: &Path, version: &str, rel: &str) -> Result<(Vec<Lod>, usize, u64)> {
    let mut lods = Vec::new();
    let (mut files, mut bytes) = (0usize, 0u64);
    let mut scale = 2;
    while largest / scale >= min {
        let out = web.join(format!("lod{scale}")).join(rel);
        if let Some(dir) = out.parent() {
            fs::create_dir_all(dir)?;
        }
        save_webp(&shrink(img, scale), &out, Some(90.0))?;
        files += 1;
        bytes += fs::metadata(&out)?.len();
        lods.push(Lod { file: format!("TreeData/{version}/web/lod{scale}/{rel}"), scale });
        scale *= 2;
    }
    Ok((lods, files, bytes))
}

/// Mip 0 of one array layer as an RGBA image.
fn decode_layer(dds: &image_dds::ddsfile::Dds, layer: u32) -> Result<RgbaImage> {
    let surface = image_dds::Surface::from_dds(dds)?;
    let rgba = surface.decode_layers_mipmaps_rgba8(layer..layer + 1, 0..1)?;
    Ok(rgba.into_image()?)
}

struct SheetOut {
    rects: BTreeMap<String, AssetRect>,
    disabled: bool,
    layers: usize,
    files: usize,
    bytes: u64,
    lods: Vec<(String, Vec<Lod>)>,
}

/// `same`: an already written sheet with identical data for the rects to point at.
fn decode_sheet(
    zst: &[u8],
    file: &str,
    names: &serde_json::Value,
    version: &str,
    web: &Path,
    same: Option<&str>,
) -> Result<SheetOut> {
    let base = file.strip_suffix(".dds.zst").unwrap_or(file);
    let disabled = base.starts_with("skills-disabled");
    let raw = zstd::decode_all(zst).context("zstd")?;
    let dds = image_dds::ddsfile::Dds::read(&raw[..]).context("dds header")?;
    let layers = dds.get_num_array_layers();
    let (w, h) = (dds.get_width(), dds.get_height());

    let mut wanted: BTreeMap<(u32, Rect), Vec<String>> = BTreeMap::new();
    for (name, pos) in names.as_object().into_iter().flatten() {
        let n = |i: usize| pos.get(i).and_then(|v| v.as_u64()).map(|v| v as u32);
        let (layer, rect) = match pos.as_u64() {
            Some(i) => (i as u32, (0, 0, w, h)),
            None => match (n(0), n(1), n(2), n(3), n(4)) {
                (Some(x), Some(y), Some(rw), Some(rh), Some(i)) if x + rw <= w && y + rh <= h => (i, (x, y, rw, rh)),
                _ => continue,
            },
        };
        if layer >= 1 && layer <= layers {
            wanted.entry((layer - 1, rect)).or_default().push(name.clone());
        }
    }

    let mut rects = BTreeMap::new();
    let mut decoded = 0usize;
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut lods = Vec::new();

    if w.max(h) <= 256 {
        let round = is_icon_sheet(base);
        let file = format!("TreeData/{version}/web/{}.webp", same.unwrap_or(base));
        let cols = (layers as f64).sqrt().ceil().max(1.0) as u32;
        let rows = layers.div_ceil(cols);
        let mut atlas = same.is_none().then(|| RgbaImage::new(cols * w, rows * h));
        let mut placed = BTreeSet::new();
        for (&(layer, (x, y, rw, rh)), layer_names) in &wanted {
            let (cx, cy) = ((layer % cols) * w, (layer / cols) * h);
            if let Some(atlas) = atlas.as_mut() {
                if placed.insert(layer) {
                    let img = decode_layer(&dds, layer).with_context(|| format!("layer {layer}"))?;
                    image::imageops::replace(atlas, &img, cx as i64, cy as i64);
                    decoded += 1;
                }
                if round {
                    mask_round(atlas, cx + x, cy + y, rw, rh);
                }
            }
            for name in layer_names {
                rects.insert(name.clone(), AssetRect { file: file.clone(), x: cx + x, y: cy + y, w: rw, h: rh, ow: rw, oh: rh, round });
            }
        }
        if let Some(atlas) = atlas {
            let out = web.join(format!("{base}.webp"));
            save_webp(&atlas, &out, None)?;
            files += 1;
            bytes += fs::metadata(&out)?.len();
            if round {
                let (list, n, b) = write_lods(&atlas, w.max(h), 16, web, version, &format!("{base}.webp"))?;
                files += n;
                bytes += b;
                lods.push((file, list));
            }
        }
    } else {
        let dir = web.join(base);
        fs::create_dir_all(&dir)?;
        for (&(layer, (x, y, rw, rh)), layer_names) in &wanted {
            let mut img = decode_layer(&dds, layer).with_context(|| format!("layer {layer}"))?;
            if (x, y, rw, rh) != (0, 0, w, h) {
                img = image::imageops::crop_imm(&img, x, y, rw, rh).to_image();
            }
            let (ow, oh) = (img.width(), img.height());
            let shared = wanted.keys().filter(|(l, _)| *l == layer).count() > 1;
            let stem = if shared { format!("{layer}-{x}-{y}-{rw}-{rh}") } else { layer.to_string() };
            let out = dir.join(format!("{stem}.webp"));
            save_webp(&img, &out, Some(82.0))?;
            decoded += 1;
            files += 1;
            bytes += fs::metadata(&out)?.len();
            let file = format!("TreeData/{version}/web/{base}/{stem}.webp");
            let (list, n, b) = write_lods(&img, img.width().max(img.height()), 64, web, version, &format!("{base}/{stem}.webp"))?;
            files += n;
            bytes += b;
            if !list.is_empty() {
                lods.push((file.clone(), list));
            }
            for name in layer_names {
                rects.insert(
                    name.clone(),
                    AssetRect { file: file.clone(), x: 0, y: 0, w: img.width(), h: img.height(), ow, oh, round: false },
                );
            }
        }
    }

    Ok(SheetOut { rects, disabled, layers: decoded, files, bytes, lods })
}

/// PoE1's sheets are plain PNG/JPG/WebP atlases whose rects are listed in
/// `sprites.lua`, so the manifest is a transcription: copy each sheet as it is
/// and key every rect by the icon path or asset name PoB uses. The `*Inactive`
/// sections are the greyscale icon variants.
pub fn build_sprites(src_tree: &Path, art_root: &Path, dest_tree: &Path, version: &str) -> Result<AssetStats> {
    let sprites = crate::lua_json::eval_file(&src_tree.join("sprites.lua"))?;
    let web = dest_tree.join("web");
    if web.exists() {
        fs::remove_dir_all(&web)?;
    }
    fs::create_dir_all(&web)?;

    let mut manifest = Manifest { version: version.to_string(), ..Default::default() };
    let mut stats = AssetStats::default();
    let Some(sections) = sprites.get("sprites").and_then(|v| v.as_object()) else {
        anyhow::bail!("sprites.lua has no sprites table");
    };
    let mut copied: BTreeMap<String, u64> = BTreeMap::new();
    let num = |r: &serde_json::Value, k: &str| r.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0).round() as u32;
    // An icon sheet is masked before it is written, so all of its rects are needed first.
    let mut icon_rects: BTreeMap<&str, BTreeSet<Rect>> = BTreeMap::new();
    let mut largest: BTreeMap<&str, u32> = BTreeMap::new();
    for (section, sheet) in sections {
        let Some(basename) = sheet.get("filename").and_then(|v| v.as_str()).map(sheet_basename) else { continue };
        let rects: Vec<Rect> = sheet
            .get("coords")
            .and_then(|v| v.as_object())
            .into_iter()
            .flatten()
            .map(|(_, r)| (num(r, "x"), num(r, "y"), num(r, "w"), num(r, "h")))
            .collect();
        let most = largest.entry(basename).or_default();
        *most = rects.iter().map(|r| r.2.max(r.3)).fold(*most, u32::max);
        if is_icon_section(section) {
            icon_rects.entry(basename).or_default().extend(rects);
        }
    }
    // A bloodline sheet repeats the ascendancy frame names with its own art.
    // PoB registers them as "<Ascendancy><Asset>" (PassiveTree.lua
    // bloodlineSpriteTypes); one sheet can serve several ascendancies.
    let bloodline_prefixes = |section: &str| -> Vec<&'static str> {
        match section {
            "trialmasterBloodline" => vec!["Trialmaster"],
            "oshabiBloodline" => vec!["Oshabi"],
            "olrothBloodline" => vec!["Olroth"],
            "lyciaBloodline" => vec!["Lycia"],
            "kingInTheMistsBloodline" => vec!["KingInTheMists"],
            "farrulBloodline" => vec!["Farrul"],
            "deliriousBloodline" => vec!["Delirious"],
            "catarinaBloodline" => vec!["Catarina"],
            "breachlordBloodline" => vec!["Breachlord"],
            "aulBloodline" => vec!["Aul"],
            "azmeriBloodline" => vec!["Azmeri", "Warden", "Warlock", "Primalist"],
            "abyssalBloodline" => vec!["Abyssal"],
            "brinerotBloodline" => vec!["Brinerot"],
            "necromanticBloodline" => vec!["Necromantic"],
            _ => Vec::new(),
        }
    };
    // A sheet this large is split into a file per sprite, so only the sprites in view decode.
    const SPLIT_PIXELS: u64 = 4_000_000;
    let mut split_images: BTreeMap<String, RgbaImage> = BTreeMap::new();
    let mut crops: BTreeMap<(String, Rect), String> = BTreeMap::new();
    for (section, sheet) in sections {
        let Some(filename) = sheet.get("filename").and_then(|v| v.as_str()) else { continue };
        let basename = sheet_basename(filename);
        let src = src_tree.join(basename);
        if !src.is_file() {
            stats.skipped.push(format!("{section}: {basename} missing"));
            continue;
        }
        let icons = icon_rects.get(basename);
        let stem = basename.rsplit_once('.').map_or(basename, |(stem, _)| stem);
        let split = icons.is_none() && image::image_dimensions(&src).is_ok_and(|(w, h)| w as u64 * h as u64 >= SPLIT_PIXELS);
        // JPG has no alpha, so a masked sheet is written as WebP.
        let out_name = if icons.is_some() { format!("{stem}.webp") } else { basename.to_string() };
        if split {
            if !split_images.contains_key(basename) {
                let img = image::open(&src).with_context(|| format!("read {}", src.display()))?.to_rgba8();
                split_images.insert(basename.to_string(), img);
            }
        } else if !copied.contains_key(&out_name) {
            let out = web.join(&out_name);
            let mut img = image::open(&src).with_context(|| format!("read {}", src.display()))?.to_rgba8();
            if let Some(rects) = icons {
                for &(x, y, w, h) in rects {
                    mask_round(&mut img, x, y, w, h);
                }
                save_webp(&img, &out, Some(82.0))?;
            } else {
                fs::copy(&src, &out).with_context(|| format!("copy {}", src.display()))?;
            }
            let len = fs::metadata(&out)?.len();
            copied.insert(out_name.clone(), len);
            stats.files += 1;
            stats.bytes += len;
            let min = if icons.is_some() { 16 } else { 64 };
            let (list, n, b) = write_lods(&img, largest.get(basename).copied().unwrap_or(0), min, &web, version, &format!("{stem}.webp"))?;
            stats.files += n;
            stats.bytes += b;
            if !list.is_empty() {
                manifest.lods.insert(format!("TreeData/{version}/web/{out_name}"), list);
            }
        }
        stats.sheets += 1;
        let file = format!("TreeData/{version}/web/{out_name}");
        let target = if section.ends_with("Inactive") { &mut manifest.disabled } else { &mut manifest.assets };
        let prefixes = bloodline_prefixes(section);
        for (name, rect) in sheet.get("coords").and_then(|v| v.as_object()).into_iter().flatten() {
            let (x, y, w, h) = (num(rect, "x"), num(rect, "y"), num(rect, "w"), num(rect, "h"));
            let own_name = prefixes.is_empty() || name.starts_with("Classes");
            if let Some(full) = full_size(art_root, name).filter(|_| own_name) {
                target.insert(name.clone(), copy_full(&full, name, (w, h), &web, version, &mut manifest.lods, &mut stats)?);
                stats.layers += 1;
                continue;
            }
            let r = if split {
                let key = (basename.to_string(), (x, y, w, h));
                let crop_file = match crops.get(&key) {
                    Some(f) => f.clone(),
                    None => {
                        let crop = image::imageops::crop_imm(&split_images[basename], x, y, w, h).to_image();
                        let rel = format!("{stem}/{}.webp", crops.len());
                        let out = web.join(&rel);
                        fs::create_dir_all(web.join(stem))?;
                        save_webp(&crop, &out, Some(90.0))?;
                        stats.files += 1;
                        stats.bytes += fs::metadata(&out)?.len();
                        let f = format!("TreeData/{version}/web/{rel}");
                        let (list, n, b) = write_lods(&crop, w.max(h), 64, &web, version, &rel)?;
                        stats.files += n;
                        stats.bytes += b;
                        if !list.is_empty() {
                            manifest.lods.insert(f.clone(), list);
                        }
                        crops.insert(key, f.clone());
                        f
                    }
                };
                AssetRect { file: crop_file, x: 0, y: 0, w, h, ow: w, oh: h, round: false }
            } else {
                AssetRect { file: file.clone(), x, y, w, h, ow: w, oh: h, round: icons.is_some() }
            };
            // Plates ("ClassesAul") keep their own names; frames are prefixed
            // so they do not replace the regular ones from frame-3.png.
            if own_name {
                target.insert(name.clone(), r);
            } else {
                for p in &prefixes {
                    target.insert(format!("{p}{name}"), r.clone());
                }
            }
            stats.layers += 1;
        }
    }
    // Standalone images PoB loads outside the sheets: the class illustrations
    // beside the version folders (LoadImage checks TreeData/ first) and the
    // jewel radius rings PassiveTreeView.lua opens by path.
    let mut standalone: Vec<(String, String)> = Vec::new();
    for name in ["BackgroundStr", "BackgroundDex", "BackgroundInt", "BackgroundStrDex", "BackgroundStrInt", "BackgroundDexInt"] {
        standalone.push((name.to_string(), format!("{name}.png")));
    }
    for name in ["EternalEmpire", "Karui", "Maraketh", "Templar", "Vaal", "Kalguuran"] {
        for i in 1..=2 {
            standalone.push((format!("{name}JewelCircle{i}"), format!("PassiveSkillScreen{name}JewelCircle{i}.png")));
        }
    }
    standalone.extend(ring_entries());
    // Timeless jewel node art lives in its own sheets (PassiveTree.lua loads
    // TreeData/legion/tree-legion.lua): icon types as zoom-level arrays, plus
    // treeAssets for the Abyss frames.
    let legion_dir = art_root.join("legion");
    let legion_table = legion_dir.join("tree-legion.lua");
    if legion_table.is_file() {
        let legion = crate::lua_json::eval_file(&legion_table)?;
        let mut sheets: Vec<(&str, &serde_json::Value)> = Vec::new();
        for (kind, entries) in legion.as_object().into_iter().flatten() {
            if kind == "treeAssets" {
                for e in entries.as_array().into_iter().flatten() {
                    sheets.push((kind, e));
                }
            } else if let Some(last) = entries.as_array().and_then(|a| a.last()) {
                sheets.push((kind, last));
            }
        }
        for (kind, sheet) in sheets {
            let Some(filename) = sheet.get("filename").and_then(|v| v.as_str()) else { continue };
            let src = legion_dir.join(filename);
            if !src.is_file() {
                stats.skipped.push(format!("legion: {filename} missing"));
                continue;
            }
            let basename = format!("legion-{filename}");
            if !copied.contains_key(&basename) {
                let out = web.join(&basename);
                fs::copy(&src, &out).with_context(|| format!("copy {}", src.display()))?;
                let len = fs::metadata(&out)?.len();
                copied.insert(basename.clone(), len);
                stats.files += 1;
                stats.bytes += len;
            }
            stats.sheets += 1;
            let file = format!("TreeData/{version}/web/{basename}");
            let target = if kind.ends_with("Inactive") { &mut manifest.disabled } else { &mut manifest.assets };
            for (name, rect) in sheet.get("coords").and_then(|v| v.as_object()).into_iter().flatten() {
                let (w, h) = (num(rect, "w"), num(rect, "h"));
                target.insert(name.clone(), AssetRect { file: file.clone(), x: num(rect, "x"), y: num(rect, "y"), w, h, ow: w, oh: h, round: false });
                stats.layers += 1;
            }
        }
    }
    copy_standalone(art_root, &web, version, &standalone, &mut manifest, &mut stats)?;
    fs::write(web.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;
    Ok(stats)
}

/// x, y, w, h in sheet pixels.
type Rect = (u32, u32, u32, u32);

/// "https://web.poecdn.com/image/passive-skill/skills-3.jpg?1540b3b6" -> "skills-3.jpg"
fn sheet_basename(filename: &str) -> &str {
    filename.rsplit('/').next().unwrap_or(filename).split('?').next().unwrap_or(filename)
}

/// The jewel radius rings PassiveTreeView.lua opens from Assets/ in both games.
fn ring_entries() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = ["ShadedOuterRing", "ShadedOuterRingFlipped", "ShadedInnerRing", "ShadedInnerRingFlipped"]
        .iter()
        .map(|name| (name.to_string(), format!("Assets/{name}.png")))
        .collect();
    v.push(("JewelRing".to_string(), "Assets/ring.png".to_string()));
    v
}

/// PoB lays out PoE1 tree art at the size of GGG's 0.3835 zoom sheets.
const SHEET_SCALE: f64 = 0.3835;

/// The game's full-size image for asset `name`, kept in `trees/<game>/full/`.
fn full_size(art_root: &Path, name: &str) -> Option<PathBuf> {
    ["webp", "png"].into_iter().map(|ext| art_root.join("full").join(format!("{name}.{ext}"))).find(|p| p.is_file())
}

/// Copies `src` to `web/<rel>` with its LODs and returns its manifest path and pixel size.
fn copy_image(src: &Path, web: &Path, version: &str, rel: &str, lods: &mut BTreeMap<String, Vec<Lod>>, stats: &mut AssetStats) -> Result<(String, u32, u32)> {
    let (w, h) = image::image_dimensions(src).with_context(|| format!("read {}", src.display()))?;
    let out = web.join(rel);
    let file = format!("TreeData/{version}/web/{rel}");
    if !out.is_file() {
        if let Some(dir) = out.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::copy(src, &out).with_context(|| format!("copy {}", src.display()))?;
        stats.files += 1;
        stats.bytes += fs::metadata(&out)?.len();
        let img = image::open(src).with_context(|| format!("read {}", src.display()))?.to_rgba8();
        let stem = rel.rsplit_once('.').map_or(rel, |(stem, _)| stem);
        let (list, n, b) = write_lods(&img, w.max(h), 64, web, version, &format!("{stem}.webp"))?;
        stats.files += n;
        stats.bytes += b;
        if !list.is_empty() {
            lods.insert(file.clone(), list);
        }
    }
    Ok((file, w, h))
}

/// A sheet rect's full-size replacement, drawn at the rect's size (`ow`, `oh`).
fn copy_full(full: &Path, name: &str, layout: (u32, u32), web: &Path, version: &str, lods: &mut BTreeMap<String, Vec<Lod>>, stats: &mut AssetStats) -> Result<AssetRect> {
    let ext = full.extension().and_then(|e| e.to_str()).unwrap_or("webp");
    let (file, w, h) = copy_image(full, web, version, &format!("full/{name}.{ext}"), lods, stats)?;
    Ok(AssetRect { file, x: 0, y: 0, w, h, ow: layout.0, oh: layout.1, round: false })
}

/// Copies images PoB opens by path (not sheet rects) into `web/` and keys them
/// by name; `entries` are (asset name, path relative to `trees/<game>/`).
fn copy_standalone(art_root: &Path, web: &Path, version: &str, entries: &[(String, String)], manifest: &mut Manifest, stats: &mut AssetStats) -> Result<()> {
    for (name, rel) in entries {
        if let Some(full) = full_size(art_root, name) {
            let (w, h) = image::image_dimensions(&full).with_context(|| format!("read {}", full.display()))?;
            let layout = |v: u32| (v as f64 * SHEET_SCALE).round() as u32;
            let r = copy_full(&full, name, (layout(w), layout(h)), web, version, &mut manifest.lods, stats)?;
            manifest.assets.insert(name.clone(), r);
            continue;
        }
        let src = art_root.join(rel);
        if !src.is_file() {
            stats.skipped.push(format!("{rel} missing"));
            continue;
        }
        let (file, w, h) = copy_image(&src, web, version, &format!("{name}.png"), &mut manifest.lods, stats)?;
        manifest.assets.insert(name.clone(), AssetRect { file, x: 0, y: 0, w, h, ow: w, oh: h, round: false });
    }
    Ok(())
}
