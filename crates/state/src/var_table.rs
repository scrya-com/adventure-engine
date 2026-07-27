//! Typed variable table.
//!
//! String-keyed → typed values. Used by Rhai scripts via
//! `set_var` / `get_var` bindings. Persists to save files.

use std::collections::HashMap;

use adventure_core::{AssetId, SmolStr};
use serde::{Deserialize, Serialize};

/// A typed value stored in [`VarTable`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", content = "v")]
pub enum VarValue {
    /// 64-bit signed integer.
    I(i64),
    /// 64-bit float.
    F(f64),
    /// String.
    S(SmolStr),
    /// Boolean.
    B(bool),
    /// Asset reference (path-hash, resolves at use time).
    Asset(AssetId),
}

impl VarValue {
    /// True if this value is truthy (non-zero, non-empty, etc.).
    pub fn truthy(&self) -> bool {
        match self {
            VarValue::I(v) => *v != 0,
            VarValue::F(v) => *v != 0.0,
            VarValue::S(s) => !s.is_empty(),
            VarValue::B(b) => *b,
            VarValue::Asset(a) => a.as_u64() != 0,
        }
    }

    /// Coerce to integer if possible.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            VarValue::I(v) => Some(*v),
            VarValue::F(v) => Some(*v as i64),
            VarValue::B(b) => Some(*b as i64),
            _ => None,
        }
    }

    /// Coerce to float if possible.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            VarValue::I(v) => Some(*v as f64),
            VarValue::F(v) => Some(*v),
            _ => None,
        }
    }

    /// Borrow as string if it is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            VarValue::S(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

/// A typed key→value table.
///
/// Keys are arbitrary strings (`"met_bob"`, `"score"`, `"current_chapter"`).
/// Values are typed via [`VarValue`]. Used by Rhai scripts through
/// `set_var` / `get_var` bindings (see `adventure-scripting`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VarTable {
    vars: HashMap<SmolStr, VarValue>,
}

impl VarTable {
    /// Empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a value, overwriting any previous.
    pub fn set(&mut self, key: impl Into<SmolStr>, value: VarValue) {
        self.vars.insert(key.into(), value);
    }

    /// Get a value (borrowed).
    pub fn get(&self, key: &str) -> Option<&VarValue> {
        self.vars.get(key)
    }

    /// Remove a key.
    pub fn remove(&mut self, key: &str) -> Option<VarValue> {
        self.vars.remove(key)
    }

    /// True if the key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// Iterate `(key, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&SmolStr, &VarValue)> {
        self.vars.iter()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Snapshot to a sorted Vec (for save files).
    pub fn to_sorted_vec(&self) -> Vec<(SmolStr, VarValue)> {
        let mut v: Vec<_> = self.vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.vars.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get() {
        let mut t = VarTable::new();
        t.set("score", VarValue::I(42));
        t.set("name", VarValue::S(SmolStr::new("Ardy")));
        t.set("alive", VarValue::B(true));
        assert_eq!(t.get("score").unwrap().as_int(), Some(42));
        assert_eq!(t.get("name").unwrap().as_str(), Some("Ardy"));
        assert!(t.get("alive").unwrap().truthy());
    }

    #[test]
    fn overwrite() {
        let mut t = VarTable::new();
        t.set("x", VarValue::I(1));
        t.set("x", VarValue::I(2));
        assert_eq!(t.get("x").unwrap().as_int(), Some(2));
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn coerce_int_from_float() {
        let v = VarValue::F(3.7);
        assert_eq!(v.as_int(), Some(3));
    }

    #[test]
    fn truthy_edge_cases() {
        assert!(!VarValue::I(0).truthy());
        assert!(VarValue::I(-1).truthy());
        assert!(!VarValue::F(0.0).truthy());
        assert!(!VarValue::S(SmolStr::new("")).truthy());
        assert!(VarValue::S(SmolStr::new("x")).truthy());
    }

    #[test]
    fn serde_roundtrip() {
        let mut t = VarTable::new();
        t.set("a", VarValue::I(1));
        t.set("b", VarValue::S(SmolStr::new("hi")));
        let json = serde_json::to_string(&t).unwrap();
        let back: VarTable = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.get("a").unwrap().as_int(), Some(1));
    }
}
