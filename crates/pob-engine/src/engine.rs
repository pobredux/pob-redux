use std::path::{Path, PathBuf};
use std::time::Instant;

use mlua::serde::{DeserializeOptions, SerializeOptions};
use mlua::{Function, Lua, LuaOptions, LuaSerdeExt, StdLib, Table, Value as LuaValue};
use serde_json::Value;

use crate::cache::{self, LuaCache};
use crate::native;
use crate::{Error, Result};

const HOST_LUA: &str = include_str!("../lua/host.lua");
const BRIDGE_LUA: &str = include_str!("../lua/bridge.lua");
const MOD_FAMILIES_LUA: &str = include_str!("../lua/mod_families.lua");

#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Directory containing PoB's `Launch.lua`, `Modules/`, `Data/`, `TreeData/`, `lua/`.
    pub pob_root: PathBuf,
    /// PoB appends `/Path of Building (PoE2)/` to this and keeps `Builds/` and
    /// `Settings.xml` under it. Pointing it at the user's Documents folder makes
    /// builds interoperate with an installed Path of Building.
    pub user_dir: PathBuf,
    /// Where compiled Lua is kept between runs. `None` compiles every source on every boot.
    pub cache_dir: Option<PathBuf>,
    /// Files the app downloads instead of shipping, by PoB path. `None` uses `<pob_root>-data` when it exists.
    pub data_dir: Option<PathBuf>,
}

/// A booted PoB instance. Not `Send`: keep it on one thread (see [`crate::EngineHandle`]).
pub struct Engine {
    lua: Lua,
    methods: Table,
    started: Instant,
    pub boot_ms: u128,
}

/// Forward-slash form without a trailing slash. Windows verbatim prefixes
/// (`\\?\C:\...`, as returned by canonicalize) are removed: Lua's package
/// searcher would otherwise substitute the `?`.
fn slash(p: &Path) -> String {
    let s = p.to_string_lossy();
    let s = s
        .strip_prefix(r"\\?\UNC\")
        .map(|rest| format!(r"\\{rest}"))
        .or_else(|| s.strip_prefix(r"\\?\").map(str::to_string))
        .unwrap_or_else(|| s.to_string());
    s.replace('\\', "/").trim_end_matches('/').to_string()
}

impl Engine {
    pub fn boot(cfg: EngineConfig) -> Result<Self> {
        let t0 = Instant::now();
        if !cfg.pob_root.join("Launch.lua").is_file() {
            return Err(Error::Other(format!(
                "PoB root {} does not contain Launch.lua",
                cfg.pob_root.display()
            )));
        }
        std::fs::create_dir_all(&cfg.user_dir)?;

        // PoB's own runtime is a full, unsandboxed LuaJIT; mirror it.
        let lua = unsafe { Lua::unsafe_new_with(StdLib::ALL, LuaOptions::default()) };

        let globals = lua.globals();
        // PoB joins GetScriptPath() onto its own paths, so the root must be
        // absolute whatever the caller passed.
        let pob_root = cfg.pob_root.canonicalize().unwrap_or_else(|_| cfg.pob_root.clone());
        let user_dir = cfg.user_dir.canonicalize().unwrap_or_else(|_| cfg.user_dir.clone());
        globals.set("__pob_root", slash(&pob_root))?;
        globals.set("__user_dir", slash(&user_dir))?;
        let sibling = || pob_root.file_name().map(|n| pob_root.with_file_name(format!("{}-data", n.to_string_lossy()))).filter(|d| d.is_dir());
        if let Some(dir) = cfg.data_dir.clone().or_else(sibling) {
            globals.set("__data_dir", slash(&dir))?;
        }
        let native = native::build_table(&lua, t0)?;
        if let Some(dir) = &cfg.cache_dir {
            let runtime: String = lua.load("return jit.version .. ' ' .. jit.arch").eval()?;
            native.set("cache", cache::table(&lua, LuaCache::open(dir, &slash(&pob_root), &runtime))?)?;
        }
        globals.set("__native", native)?;

        lua.load(HOST_LUA).set_name("host.lua").exec()?;
        let boot: Function = globals.get("__pob_boot")?;
        boot.call::<()>(())?;

        let families: Table = lua.load(MOD_FAMILIES_LUA).set_name("mod_families.lua").eval()?;
        globals.set("__mod_families", families)?;
        let methods: Table = lua.load(BRIDGE_LUA).set_name("bridge.lua").eval()?;

        let boot_ms = t0.elapsed().as_millis();
        log::info!("pob-engine booted in {boot_ms} ms");
        Ok(Self { lua, methods, started: t0, boot_ms })
    }

    pub fn uptime_ms(&self) -> u128 {
        self.started.elapsed().as_millis()
    }

    pub fn method_names(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for pair in self.methods.pairs::<String, LuaValue>() {
            let (k, _) = pair?;
            out.push(k);
        }
        out.sort();
        Ok(out)
    }

    /// Invoke a bridge method with JSON params; returns its JSON result.
    pub fn call(&self, method: &str, params: &Value) -> Result<Value> {
        let f: Option<Function> = self.methods.get(method)?;
        let Some(f) = f else {
            return Err(Error::Other(format!("unknown engine method '{method}'")));
        };
        let ser = SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false);
        let lua_params = match params {
            Value::Null => LuaValue::Nil,
            other => self.lua.to_value_with(other, ser)?,
        };
        let ret: LuaValue = f.call(lua_params)?;
        let de = DeserializeOptions::new()
            .deny_unsupported_types(false)
            .deny_recursive_tables(false);
        let out: Value = match ret {
            LuaValue::Nil => Value::Null,
            v => self.lua.from_value_with(v, de)?,
        };
        Ok(out)
    }

    /// Takes the bridge's background work off PoB's frames, to be done by `idle_step`. True when there is any.
    pub fn hold_idle_work(&self) -> bool {
        matches!(self.call("idle_step", &serde_json::json!({ "hold": true })), Ok(Value::Bool(true)))
    }

    /// One step of the bridge's background work. True while more remains.
    pub fn idle_step(&self) -> bool {
        matches!(self.call("idle_step", &Value::Null), Ok(Value::Bool(true)))
    }

    /// Run a raw Lua chunk. Debug/REPL use only.
    pub fn eval(&self, code: &str) -> Result<Value> {
        let ret: LuaValue = self.lua.load(code).eval()?;
        let de = DeserializeOptions::new()
            .deny_unsupported_types(false)
            .deny_recursive_tables(false);
        Ok(match ret {
            LuaValue::Nil => Value::Null,
            v => self.lua.from_value_with(v, de)?,
        })
    }
}
