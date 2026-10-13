//! Loading Lua data files (quest data exports, SavedVariables) into plain Rust values.
//!
//! Lua tables keep their keys so that sparse maps (`{[12] = ...}`) and positional rows
//! with holes (`{"a", nil, 3}`) stay distinguishable.

use anyhow::{Context, Result};
use mlua::{ChunkMode, HookTriggers, Lua, LuaOptions, StdLib, Table, Value as LV, VmState};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

const MEMORY_LIMIT: usize = 1 << 30;
/// Instructions a file may run: a real SavedVariables runs about 0.05 per byte.
const BASE_INSTRUCTIONS: u64 = 10_000_000;
const INSTRUCTIONS_PER_BYTE: u64 = 100;
const HOOK_STEP: u32 = 10_000;
/// The addon's SavedVariables nest less than ten tables deep.
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Int(i64),
    Str(String),
}

#[derive(Debug, Clone)]
pub enum Val {
    Nil,
    Bool(bool),
    Num(f64),
    Str(String),
    Table(BTreeMap<Key, Val>),
}

impl Val {
    pub fn get(&self, i: i64) -> &Val {
        match self {
            Val::Table(t) => t.get(&Key::Int(i)).unwrap_or(&Val::Nil),
            _ => &Val::Nil,
        }
    }

    pub fn field(&self, name: &str) -> &Val {
        match self {
            Val::Table(t) => t.get(&Key::Str(name.to_owned())).unwrap_or(&Val::Nil),
            _ => &Val::Nil,
        }
    }

    pub fn int(&self) -> Option<i64> {
        match self {
            Val::Num(n) => Some(*n as i64),
            Val::Str(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn num(&self) -> Option<f64> {
        match self {
            Val::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn str(&self) -> Option<&str> {
        match self {
            Val::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Val::Nil)
    }

    /// Integer-keyed entries in key order (handles holes).
    pub fn entries(&self) -> Vec<(i64, &Val)> {
        match self {
            Val::Table(t) => t
                .iter()
                .filter_map(|(k, v)| match k {
                    Key::Int(i) => Some((*i, v)),
                    Key::Str(_) => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Values of a list, ignoring holes.
    pub fn list(&self) -> Vec<&Val> {
        self.entries().into_iter().map(|(_, v)| v).collect()
    }

    pub fn ints(&self) -> Vec<i64> {
        self.list().into_iter().filter_map(Val::int).collect()
    }

    pub fn str_entries(&self) -> Vec<(&str, &Val)> {
        match self {
            Val::Table(t) => t
                .iter()
                .filter_map(|(k, v)| match k {
                    Key::Str(s) => Some((s.as_str(), v)),
                    Key::Int(_) => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::Value as J;
        match self {
            Val::Nil => J::Null,
            Val::Bool(b) => J::Bool(*b),
            Val::Num(n) if n.fract() == 0.0 && n.abs() < 9e15 => J::from(*n as i64),
            Val::Num(n) => J::from(*n),
            Val::Str(s) => J::String(s.clone()),
            Val::Table(t) => {
                let dense = t.keys().enumerate().all(|(i, k)| *k == Key::Int(i as i64 + 1));
                if dense {
                    J::Array(t.values().map(Val::to_json).collect())
                } else {
                    J::Object(
                        t.iter()
                            .map(|(k, v)| {
                                let k = match k {
                                    Key::Int(i) => i.to_string(),
                                    Key::Str(s) => s.clone(),
                                };
                                (k, v.to_json())
                            })
                            .collect(),
                    )
                }
            }
        }
    }
}

/// Converts a Lua value; tables nested too deep or met twice (cycles, shared tables that would
/// expand exponentially) are refused.
pub fn convert(v: &LV) -> Result<Val> {
    convert_at(v, 0, &mut HashSet::new())
}

fn convert_at(v: &LV, depth: usize, seen: &mut HashSet<usize>) -> Result<Val> {
    Ok(match v {
        LV::Boolean(b) => Val::Bool(*b),
        LV::Integer(i) => Val::Num(*i as f64),
        LV::Number(n) => Val::Num(*n),
        LV::String(s) => Val::Str(s.to_string_lossy().clone()),
        LV::Table(t) => {
            anyhow::ensure!(depth < MAX_DEPTH, "Lua tables nested too deep");
            anyhow::ensure!(seen.insert(t.to_pointer() as usize), "Lua table met twice");
            let mut map = BTreeMap::new();
            for pair in t.clone().pairs::<LV, LV>() {
                let (k, v) = pair?;
                let key = match k {
                    LV::Integer(i) => Key::Int(i),
                    LV::Number(n) if n.fract() == 0.0 => Key::Int(n as i64),
                    LV::String(s) => Key::Str(s.to_string_lossy().clone()),
                    other => Key::Str(format!("{other:?}")),
                };
                map.insert(key, convert_at(&v, depth + 1, seen)?);
            }
            Val::Table(map)
        }
        _ => Val::Nil,
    })
}

/// A state for untrusted data files: no libraries, bounded memory.
pub fn new_lua() -> Lua {
    let lua = Lua::new_with(StdLib::NONE, LuaOptions::default()).expect("creating Lua state");
    lua.set_memory_limit(MEMORY_LIMIT).expect("limiting Lua memory");
    lua
}

/// Text only (Lua 5.1 bytecode is unverified, so it can corrupt memory), an empty environment
/// so the file reaches none of the base library (`dofile`, `loadfile`, …), and a budget of
/// instructions after the file's size.
fn sandboxed(lua: &Lua, path: &Path) -> Result<(mlua::Chunk<'static>, Table)> {
    let src = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let budget = BASE_INSTRUCTIONS + INSTRUCTIONS_PER_BYTE * src.len() as u64;
    let run = AtomicU64::new(0);
    lua.set_hook(HookTriggers::new().every_nth_instruction(HOOK_STEP), move |_, _| {
        if run.fetch_add(u64::from(HOOK_STEP), Ordering::Relaxed) > budget {
            return Err(mlua::Error::runtime("Lua file runs too long"));
        }
        Ok(VmState::Continue)
    })?;
    let env = lua.create_table()?;
    let chunk = lua
        .load(src)
        .set_name(path.display().to_string())
        .set_mode(ChunkMode::Text)
        .set_environment(env.clone());
    Ok((chunk, env))
}

/// Run a Lua file and convert its return value.
pub fn eval_file(lua: &Lua, path: &Path) -> Result<Val> {
    let (chunk, _) = sandboxed(lua, path)?;
    let v: LV = chunk.eval().with_context(|| format!("evaluating {}", path.display()))?;
    convert(&v)
}

/// Run a SavedVariables-like file (assigns globals) and return the named globals.
pub fn load_globals(lua: &Lua, path: &Path, names: &[&str]) -> Result<Vec<Val>> {
    let (chunk, env) = sandboxed(lua, path)?;
    chunk.exec().with_context(|| format!("executing {}", path.display()))?;
    names.iter().map(|n| convert(&env.get::<LV>(*n)?)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(src: &[u8]) -> Result<Vec<Val>> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let file = std::env::temp_dir().join(format!("fg-lua-{}-{n}.lua", std::process::id()));
        std::fs::write(&file, src)?;
        let out = load_globals(&new_lua(), &file, &["DB"]);
        std::fs::remove_file(&file)?;
        out
    }

    #[test]
    fn reads_saved_variables() {
        let v = load(b"DB = { [\"a\"] = { 1, nil, 3 }, [12] = true }").unwrap();
        assert_eq!(v[0].field("a").get(3).int(), Some(3));
    }

    #[test]
    fn reaches_no_library() {
        for src in [
            "os.execute('true')",
            "io.open('/etc/passwd')",
            "dofile('/etc/passwd')",
            "loadfile('/etc/passwd')",
            "require('os')",
            "string.rep('x', 1)",
        ] {
            assert!(load(src.as_bytes()).is_err(), "{src}");
        }
    }

    #[test]
    fn refuses_bytecode() {
        assert!(load(b"\x1bLua\x51\x00").is_err());
    }

    #[test]
    fn bounds_instructions() {
        assert!(load(b"while true do end").is_err());
    }

    #[test]
    fn refuses_cycles_and_shared_tables() {
        assert!(load(b"DB = {} DB.self = DB").is_err());
        assert!(load(b"local a = {} for i = 1, 60 do a = {a, a} end DB = a").is_err());
        assert!(load(b"local a = {} for i = 1, 100000 do a = {a} end DB = a").is_err());
    }

    #[test]
    fn bounds_memory() {
        assert!(load(b"local t = {} for i = 1, 1e10 do t[i] = i end").is_err());
    }
}
