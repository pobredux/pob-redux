//! Rust implementations of the SimpleGraphic host functions PoB needs that
//! cannot be no-op stubs: compression, time, and filesystem access.

use std::io::{Read, Write};
use std::path::Path;
use std::time::{Instant, UNIX_EPOCH};

use flate2::read::{DeflateDecoder, ZlibDecoder};
use flate2::write::ZlibEncoder;
use flate2::Compression;
use mlua::{Lua, LuaSerdeExt, LuaString, Result as LuaResult, Table, Value};

pub fn build_table(lua: &Lua, epoch: Instant) -> LuaResult<Table> {
    let t = lua.create_table()?;

    t.set("null", lua.null())?;

    t.set(
        "array",
        lua.create_function(|lua, tbl: Table| {
            tbl.set_metatable(Some(lua.array_metatable()))?;
            Ok(tbl)
        })?,
    )?;

    t.set(
        "time",
        lua.create_function(move |_, ()| Ok(epoch.elapsed().as_millis() as f64))?,
    )?;

    t.set(
        "log",
        lua.create_function(|_, (level, msg): (String, LuaString)| {
            let msg = msg.to_string_lossy();
            match level.as_str() {
                "error" => log::error!(target: "pob", "{msg}"),
                "warn" => log::warn!(target: "pob", "{msg}"),
                "info" => log::info!(target: "pob", "{msg}"),
                _ => log::debug!(target: "pob", "{msg}"),
            }
            Ok(())
        })?,
    )?;

    // SimpleGraphic's Deflate/Inflate are zlib-wrapped (PoB share codes are
    // base64url(zlib(xml)), 0x78 0x9C header).
    t.set(
        "deflate",
        lua.create_function(|lua, data: LuaString| {
            let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
            let res = enc.write_all(&data.as_bytes()).and_then(|_| enc.finish());
            match res {
                Ok(out) => Ok(Value::String(lua.create_string(&out)?)),
                Err(e) => {
                    log::warn!("deflate failed: {e}");
                    Ok(Value::Nil)
                }
            }
        })?,
    )?;
    t.set(
        "inflate",
        lua.create_function(|lua, data: LuaString| {
            let bytes = data.as_bytes();
            let mut out = Vec::new();
            if ZlibDecoder::new(&bytes[..]).read_to_end(&mut out).is_ok() {
                return Ok(Value::String(lua.create_string(&out)?));
            }
            // Share codes from earlier builds of this app are raw deflate, no zlib header.
            out.clear();
            let mut dec = DeflateDecoder::new(&bytes[..]);
            match dec.read_to_end(&mut out) {
                Ok(_) => Ok(Value::String(lua.create_string(&out)?)),
                Err(e) => {
                    log::warn!("inflate failed: {e}");
                    Ok(Value::Nil)
                }
            }
        })?,
    )?;

    // Blocking GET for the lcurl.safe shim (PoB's trade-stats download). Runs
    // on the engine thread; callers are one-shot cache fills.
    t.set(
        "http_get",
        lua.create_function(|lua, (url, ua): (String, Option<String>)| {
            crate::install_tls_provider();
            let client = match reqwest::blocking::Client::builder()
                .user_agent(ua.unwrap_or_else(|| "Path of Building".into()))
                .timeout(std::time::Duration::from_secs(30))
                .build()
            {
                Ok(c) => c,
                Err(e) => return Ok((Value::Nil, Some(e.to_string()))),
            };
            match client.get(&url).send() {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        return Ok((Value::Nil, Some(format!("HTTP {status}"))));
                    }
                    match resp.bytes() {
                        Ok(b) => Ok((Value::String(lua.create_string(&b)?), None)),
                        Err(e) => Ok((Value::Nil, Some(e.to_string()))),
                    }
                }
                Err(e) => Ok((Value::Nil, Some(e.to_string()))),
            }
        })?,
    )?;

    t.set(
        "make_dir",
        lua.create_function(|_, path: String| match std::fs::create_dir_all(&path) {
            Ok(()) => Ok((true, None)),
            Err(e) => Ok((false, Some(e.to_string()))),
        })?,
    )?;
    t.set(
        "remove_dir",
        lua.create_function(|_, (path, recurse): (String, Option<bool>)| {
            let r = if recurse.unwrap_or(false) {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_dir(&path)
            };
            Ok(r.is_ok())
        })?,
    )?;

    // Expands a spec like "C:/x/Builds/*.xml" into [{name,size,mtime}, ...].
    // Lists directories instead of files when `dirs` is true.
    t.set(
        "file_search",
        lua.create_function(|lua, (spec, dirs): (String, Option<bool>)| {
            let want_dirs = dirs.unwrap_or(false);
            let spec = spec.replace('\\', "/");
            let (dir, pat) = match spec.rfind('/') {
                Some(i) => (&spec[..i], &spec[i + 1..]),
                None => (".", spec.as_str()),
            };
            let Ok(pattern) = glob::Pattern::new(pat) else {
                return Ok(Value::Nil);
            };
            let Ok(rd) = std::fs::read_dir(dir) else {
                return Ok(Value::Nil);
            };
            let mut entries: Vec<(String, u64, f64)> = Vec::new();
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Ok(md) = e.metadata() else { continue };
                if md.is_dir() != want_dirs || !pattern.matches(&name) {
                    continue;
                }
                let mtime = md
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                entries.push((name, md.len(), mtime));
            }
            if entries.is_empty() {
                return Ok(Value::Nil);
            }
            entries.sort_by_key(|e| e.0.to_lowercase());
            let out = lua.create_table()?;
            for (i, (name, size, mtime)) in entries.into_iter().enumerate() {
                let row = lua.create_table()?;
                row.set("name", name)?;
                row.set("size", size)?;
                row.set("mtime", mtime)?;
                out.set(i + 1, row)?;
            }
            Ok(Value::Table(out))
        })?,
    )?;

    t.set(
        "file_exists",
        lua.create_function(|_, path: String| Ok(Path::new(&path).exists()))?,
    )?;

    Ok(t)
}
