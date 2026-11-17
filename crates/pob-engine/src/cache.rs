//! Keeps the Lua sources a boot loads as LuaJIT bytecode in one file, so the next boot reads that file and parses nothing.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::UNIX_EPOCH;

use mlua::{Lua, LuaString, Result as LuaResult, Table};

const PACK: &str = "lua.pack";
const KEPT_INSTALLS: usize = 8;

static TEMP_SEQ: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy)]
struct Stamp {
    size: u64,
    mtime: u64,
    crc: u32,
}

impl Stamp {
    fn same_content(&self, other: &Stamp) -> bool {
        self.size == other.size && self.crc == other.crc
    }
}

struct Packed {
    stamp: Stamp,
    start: usize,
    len: usize,
    code_crc: u32,
    used: bool,
}

pub struct LuaCache {
    root: String,
    dir: PathBuf,
    header: String,
    pack: Vec<u8>,
    packed: HashMap<String, Packed>,
    pending: HashMap<String, Stamp>,
    compiled: HashMap<String, (Stamp, Vec<u8>)>,
    seen: HashSet<String>,
    fingerprint: u64,
    changed: bool,
}

fn file_times(path: &Path) -> Option<(u64, u64)> {
    let md = std::fs::metadata(path).ok()?;
    let mtime = md.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos() as u64;
    Some((md.len(), mtime))
}

fn write_atomic(path: &Path, parts: &[&[u8]]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    replace_file(path, parts, false)
}

pub(crate) fn replace_file(path: &Path, parts: &[&[u8]], durable: bool) -> std::io::Result<()> {
    let seq = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("tmp{}-{seq}", std::process::id()));
    let mut file = match std::fs::File::create_new(&tmp) {
        // A process that died mid-write under a reused PID leaves this name behind.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(&tmp)?;
            std::fs::File::create_new(&tmp)?
        }
        file => file?,
    };
    let written = (|| {
        #[cfg(unix)]
        if durable {
            if let Ok(metadata) = path.metadata() {
                file.set_permissions(metadata.permissions())?;
            }
        }
        parts.iter().try_for_each(|part| file.write_all(part))?;
        if durable { file.sync_all() } else { Ok(()) }
    })();
    drop(file);
    written.and_then(|()| std::fs::rename(&tmp, path)).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

fn next_line<'a>(data: &'a [u8], pos: &mut usize) -> Option<&'a str> {
    let end = *pos + data.get(*pos..)?.iter().position(|&b| b == b'\n')?;
    let line = std::str::from_utf8(&data[*pos..end]).ok()?;
    *pos = end + 1;
    Some(line)
}

fn read_pack(path: &Path, header: &str) -> Option<(Vec<u8>, HashMap<String, Packed>)> {
    let data = std::fs::read(path).ok()?;
    let mut pos = 0;
    if next_line(&data, &mut pos)? != header {
        return None;
    }
    let count: usize = next_line(&data, &mut pos)?.parse().ok()?;
    let mut rows = Vec::new();
    let mut total = 0usize;
    for _ in 0..count {
        let mut cols = next_line(&data, &mut pos)?.split('\t');
        let rel = cols.next()?.to_string();
        let stamp = Stamp { size: cols.next()?.parse().ok()?, mtime: cols.next()?.parse().ok()?, crc: cols.next()?.parse().ok()? };
        let len: usize = cols.next()?.parse().ok()?;
        let code_crc: u32 = cols.next()?.parse().ok()?;
        rows.push((rel, Packed { stamp, start: total, len, code_crc, used: false }));
        total = total.checked_add(len)?;
    }
    if data.len() - pos != total {
        return None;
    }
    let packed = rows.into_iter().map(|(rel, p)| (rel, Packed { start: pos + p.start, ..p })).collect();
    Some((data, packed))
}

// An AppImage mounts at a new path on every launch (`APPDIR`), so that part of the root is left out of the key.
fn install_key(root: &str, appdir: Option<&str>) -> String {
    let inside = appdir.map(|dir| dir.trim_end_matches('/')).filter(|dir| !dir.is_empty()).and_then(|dir| root.strip_prefix(dir));
    format!("{:08x}", crc32fast::hash(inside.unwrap_or(root).as_bytes()))
}

impl LuaCache {
    /// `runtime` names the LuaJIT build; bytecode from another build is dropped.
    pub fn open(cache_dir: &Path, root: &str, runtime: &str) -> Self {
        let dir = cache_dir.join(install_key(root, std::env::var("APPDIR").ok().as_deref()));
        let header = format!("pob-lua-pack 1 {runtime}");
        let (pack, packed) = read_pack(&dir.join(PACK), &header).unwrap_or_default();
        Self {
            root: root.to_string(),
            dir,
            header,
            pack,
            packed,
            pending: HashMap::new(),
            compiled: HashMap::new(),
            seen: HashSet::new(),
            fingerprint: 0,
            changed: false,
        }
    }

    fn rel(&self, abs: &str) -> Option<String> {
        let rest = abs.strip_prefix(self.root.as_str())?.strip_prefix(['/', '\\'])?;
        Some(rest.replace('\\', "/"))
    }

    // An unchanged size and modified time vouch for the checksum, so a warm boot reads no source.
    fn stamp(&self, abs: &str, rel: &str) -> Option<Stamp> {
        let (size, mtime) = file_times(Path::new(abs))?;
        if let Some(known) = self.packed.get(rel).map(|p| p.stamp) {
            if known.size == size && known.mtime == mtime {
                return Some(known);
            }
        }
        let crc = crc32fast::hash(&std::fs::read(abs).ok()?);
        Some(Stamp { size, mtime, crc })
    }

    fn lookup(&mut self, abs: &str) -> Option<&[u8]> {
        let rel = self.rel(abs)?;
        if self.compiled.contains_key(&rel) {
            return self.compiled.get(&rel).map(|(_, code)| code.as_slice());
        }
        let stamp = self.stamp(abs, &rel)?;
        if self.seen.insert(rel.clone()) {
            let mut h = DefaultHasher::new();
            (&rel, stamp.size, stamp.crc).hash(&mut h);
            self.fingerprint = self.fingerprint.wrapping_add(h.finish());
        }
        // LuaJIT does not verify bytecode, so code that fails its checksum is compiled again.
        let intact = self.packed.get(&rel).is_some_and(|p| p.stamp.same_content(&stamp) && crc32fast::hash(&self.pack[p.start..p.start + p.len]) == p.code_crc);
        if !intact {
            self.packed.remove(&rel);
            self.pending.insert(rel, stamp);
            return None;
        }
        let p = self.packed.get_mut(&rel)?;
        self.changed |= p.stamp.mtime != stamp.mtime;
        p.stamp = stamp;
        p.used = true;
        Some(&self.pack[p.start..p.start + p.len])
    }

    fn store(&mut self, abs: &str, code: &[u8]) {
        let Some(rel) = self.rel(abs) else { return };
        let Some(stamp) = self.pending.remove(&rel).or_else(|| self.packed.remove(&rel).map(|p| p.stamp)) else { return };
        self.compiled.insert(rel, (stamp, code.to_vec()));
        self.changed = true;
    }

    // The pack keeps only what this boot loaded, so code for sources that changed or left does not pile up.
    fn finish(&mut self) {
        if self.changed {
            let kept: Vec<(&String, &Packed)> = self.packed.iter().filter(|(_, p)| p.used).collect();
            let mut head = format!("{}\n{}\n", self.header, kept.len() + self.compiled.len());
            for (rel, p) in &kept {
                let _ = writeln!(head, "{rel}\t{}\t{}\t{}\t{}\t{}", p.stamp.size, p.stamp.mtime, p.stamp.crc, p.len, p.code_crc);
            }
            for (rel, (stamp, code)) in &self.compiled {
                let _ = writeln!(head, "{rel}\t{}\t{}\t{}\t{}\t{}", stamp.size, stamp.mtime, stamp.crc, code.len(), crc32fast::hash(code));
            }
            let mut parts: Vec<&[u8]> = vec![head.as_bytes()];
            parts.extend(kept.iter().map(|(_, p)| &self.pack[p.start..p.start + p.len]));
            parts.extend(self.compiled.values().map(|(_, code)| code.as_slice()));
            match write_atomic(&self.dir.join(PACK), &parts) {
                Ok(()) => self.prune(),
                Err(e) => log::warn!("lua cache: {} not written, so the next boot compiles again: {e}", self.dir.join(PACK).display()),
            }
        }
        self.changed = false;
        self.pack = Vec::new();
        self.packed = HashMap::new();
        self.compiled = HashMap::new();
    }

    // A root that moves between launches leaves a pack behind each time, so only the newest few installs keep one.
    fn prune(&self) {
        let Some(read) = self.dir.parent().and_then(|parent| std::fs::read_dir(parent).ok()) else { return };
        let mut installs: Vec<_> = read.flatten().filter_map(|e| Some((e.path().join(PACK).metadata().ok()?.modified().ok()?, e.path()))).collect();
        installs.sort();
        for (_, dir) in installs.iter().rev().skip(KEPT_INSTALLS).filter(|(_, dir)| *dir != self.dir) {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

pub fn table(lua: &Lua, cache: LuaCache) -> LuaResult<Table> {
    let cache = Rc::new(RefCell::new(cache));
    let t = lua.create_table()?;

    let c = cache.clone();
    t.set(
        "lookup",
        lua.create_function(move |lua, abs: LuaString| {
            let Ok(abs) = abs.to_str() else { return Ok(None) };
            c.borrow_mut().lookup(&abs).map(|code| lua.create_string(code)).transpose()
        })?,
    )?;

    let c = cache.clone();
    t.set(
        "store",
        lua.create_function(move |_, (abs, code): (LuaString, LuaString)| {
            if let Ok(abs) = abs.to_str() {
                c.borrow_mut().store(&abs, &code.as_bytes());
            }
            Ok(())
        })?,
    )?;

    let c = cache.clone();
    t.set(
        "finish",
        lua.create_function(move |_, ()| {
            c.borrow_mut().finish();
            Ok(())
        })?,
    )?;

    let c = cache.clone();
    t.set(
        "fingerprint",
        lua.create_function(move |_, ()| Ok(format!("{:016x}", c.borrow().fingerprint)))?,
    )?;

    let c = cache.clone();
    t.set(
        "read",
        lua.create_function(move |lua, name: String| {
            std::fs::read(c.borrow().dir.join(name)).ok().map(|data| lua.create_string(data)).transpose()
        })?,
    )?;

    t.set(
        "write",
        lua.create_function(move |_, (name, data): (String, LuaString)| {
            let path = cache.borrow().dir.join(name);
            if let Err(e) = write_atomic(&path, &[&data.as_bytes()]) {
                log::debug!("lua cache: {} not written: {e}", path.display());
            }
            Ok(())
        })?,
    )?;

    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn scratch(name: &str) -> (PathBuf, PathBuf, String) {
        let dir = std::env::temp_dir().join(format!("pob-lua-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("root")).unwrap();
        let source = dir.join("root").join("a.lua");
        std::fs::write(&source, "return 1").unwrap();
        let abs = source.to_string_lossy().replace('\\', "/");
        (dir, source, abs)
    }

    fn open(dir: &Path, runtime: &str) -> LuaCache {
        let root = dir.join("root").to_string_lossy().replace('\\', "/");
        LuaCache::open(&dir.join("cache"), &root, runtime)
    }

    fn compiled(dir: &Path, abs: &str) -> u64 {
        let mut cache = open(dir, "test");
        assert!(cache.lookup(abs).is_none());
        cache.store(abs, b"one");
        let fingerprint = cache.fingerprint;
        cache.finish();
        fingerprint
    }

    #[test]
    fn serves_stored_code_until_the_source_changes() {
        let (dir, source, abs) = scratch("content");
        let fingerprint = compiled(&dir, &abs);

        let mut cache = open(&dir, "test");
        assert_eq!(cache.lookup(&abs), Some(&b"one"[..]));
        assert_eq!(cache.fingerprint, fingerprint);
        assert!(!cache.changed);

        std::fs::write(&source, "return 2").unwrap();
        // Linux file times tick every few ms, so a rewrite this quick can keep the old mtime.
        let later = SystemTime::now() + Duration::from_secs(60);
        std::fs::File::options().write(true).open(&source).unwrap().set_modified(later).unwrap();
        let mut cache = open(&dir, "test");
        assert!(cache.lookup(&abs).is_none());
        assert_ne!(cache.fingerprint, fingerprint);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keeps_code_when_only_the_modified_time_changes() {
        let (dir, source, abs) = scratch("mtime");
        let fingerprint = compiled(&dir, &abs);

        let later = SystemTime::now() + Duration::from_secs(60);
        std::fs::File::options().write(true).open(&source).unwrap().set_modified(later).unwrap();
        let mut cache = open(&dir, "test");
        assert_eq!(cache.lookup(&abs), Some(&b"one"[..]));
        assert_eq!(cache.fingerprint, fingerprint);
        assert!(cache.changed);
        cache.finish();

        let mut cache = open(&dir, "test");
        assert_eq!(cache.lookup(&abs), Some(&b"one"[..]));
        assert!(!cache.changed);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn drops_code_from_another_runtime() {
        let (dir, _, abs) = scratch("runtime");
        compiled(&dir, &abs);

        let mut cache = open(&dir, "other");
        assert!(cache.lookup(&abs).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compiles_again_when_stored_code_is_damaged() {
        let (dir, _, abs) = scratch("damage");
        compiled(&dir, &abs);

        let pack = open(&dir, "test").dir.join(PACK);
        let mut whole = std::fs::read(&pack).unwrap();
        *whole.last_mut().unwrap() ^= 1;
        std::fs::write(&pack, &whole).unwrap();
        let mut cache = open(&dir, "test");
        assert!(cache.lookup(&abs).is_none());
        cache.store(&abs, b"one");
        cache.finish();

        let mut cache = open(&dir, "test");
        assert_eq!(cache.lookup(&abs), Some(&b"one"[..]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keys_an_appimage_by_the_path_inside_its_mount() {
        let first = install_key("/tmp/.mount_one/usr/lib/app/pob", Some("/tmp/.mount_one"));
        let second = install_key("/tmp/.mount_two/usr/lib/app/pob", Some("/tmp/.mount_two/"));
        assert_eq!(first, second);
        assert_ne!(first, install_key("/tmp/.mount_one/usr/lib/app/pob1", Some("/tmp/.mount_one")));
        assert_eq!(install_key("/opt/app/pob", Some("/tmp/.mount_one")), install_key("/opt/app/pob", None));
    }

    #[test]
    fn keeps_packs_for_the_newest_installs_only() {
        let (dir, _, abs) = scratch("prune");
        let long_ago = SystemTime::now() - Duration::from_secs(86_400);
        for age in 0..10u64 {
            let old = dir.join("cache").join(format!("old{age}"));
            std::fs::create_dir_all(&old).unwrap();
            std::fs::write(old.join(PACK), b"").unwrap();
            let seen = long_ago - Duration::from_secs(age * 60);
            std::fs::File::options().write(true).open(old.join(PACK)).unwrap().set_modified(seen).unwrap();
        }
        compiled(&dir, &abs);

        let left: Vec<String> = std::fs::read_dir(dir.join("cache")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(left.len(), KEPT_INSTALLS);
        assert!(!left.iter().any(|name| ["old7", "old8", "old9"].contains(&name.as_str())));
        assert!(open(&dir, "test").lookup(&abs).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_a_cut_off_pack() {
        let (dir, _, abs) = scratch("cut");
        compiled(&dir, &abs);

        let pack = open(&dir, "test").dir.join(PACK);
        let whole = std::fs::read(&pack).unwrap();
        std::fs::write(&pack, &whole[..whole.len() - 1]).unwrap();
        let mut cache = open(&dir, "test");
        assert!(cache.lookup(&abs).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
