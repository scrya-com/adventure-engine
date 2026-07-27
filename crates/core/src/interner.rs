//! String interning for tags, names, and other short identifiers.
//!
//! Mirrors UE's [`FName`](Core/Containers/NameTypes.h) — interned strings
//! compared by symbol id rather than byte content. The [`Interner`] type is
//! the engine-wide store; [`Symbol`] is the lightweight handle.
//!
//! [`Tags`](crate::Interner) is shared across the engine via a single
//! `once_cell` global, accessible through [`Interner::shared()`].

use std::sync::OnceLock;

use string_interner::{DefaultSymbol, StringInterner};

/// An interned-string symbol. Cheap to copy (4 bytes), comparable by value.
pub type Symbol = DefaultSymbol;

/// Engine-wide string interner.
///
/// Stores each unique string once and hands out [`Symbol`]s that can be
/// resolved back to `&str` cheaply. Used by [`Tag`](crate::interner::Symbol)
/// analogs (see `adventure-state::Tag`) and any other identifier that
/// appears many times.
pub struct Interner {
    inner: StringInterner,
}

impl Interner {
    /// Create an empty interner.
    pub fn new() -> Self {
        Self {
            inner: StringInterner::new(),
        }
    }

    /// Intern a string, returning its [`Symbol`].
    ///
    /// If the string is already interned, returns the existing symbol.
    /// Otherwise inserts it and returns a fresh one.
    pub fn intern(&mut self, s: &str) -> Symbol {
        self.inner.get_or_intern(s)
    }

    /// Look up an existing symbol without inserting.
    pub fn get(&self, s: &str) -> Option<Symbol> {
        self.inner.get(s)
    }

    /// Resolve a symbol back to its `&str`.
    pub fn resolve(&self, sym: Symbol) -> Option<&str> {
        self.inner.resolve(sym)
    }

    /// Number of unique strings interned.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the interner has any strings.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

static SHARED: OnceLock<std::sync::Mutex<Interner>> = OnceLock::new();

/// Access the engine-wide shared interner.
///
/// Returns a `MutexGuard` — fine for occasional intern/resolve calls.
/// For hot paths, intern once at load time and store the resulting
/// [`Symbol`] for reuse.
pub fn shared() -> std::sync::MutexGuard<'static, Interner> {
    SHARED
        .get_or_init(|| std::sync::Mutex::new(Interner::new()))
        .lock()
        .expect("interner mutex poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_dedupes() {
        let mut i = Interner::new();
        let a = i.intern("State.NPC.Bob.Met");
        let b = i.intern("State.NPC.Bob.Met");
        assert_eq!(a, b);
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn intern_distinct() {
        let mut i = Interner::new();
        let a = i.intern("foo");
        let b = i.intern("bar");
        assert_ne!(a, b);
        assert_eq!(i.len(), 2);
    }

    #[test]
    fn resolve_roundtrip() {
        let mut i = Interner::new();
        let s = i.intern("hello");
        assert_eq!(i.resolve(s), Some("hello"));
    }

    #[test]
    fn get_does_not_insert() {
        let mut i = Interner::new();
        assert_eq!(i.get("nope"), None);
        let _ = i.intern("yes");
        assert!(i.get("yes").is_some());
        assert_eq!(i.len(), 1);
    }
}
