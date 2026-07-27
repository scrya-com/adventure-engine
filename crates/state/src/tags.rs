//! Hierarchical tags — mirrors UE's `FGameplayTag`.
//!
//! Tags are dot-separated hierarchical strings like `State.NPC.Bob.Met` or
//! `State.Door.Cellar.Locked`. They're used for boolean state flags across
//! the engine — "met NPC X", "opened door Y", "completed quest Z".

use std::fmt;

use adventure_core::SmolStr;
use serde::{Deserialize, Serialize};

/// A hierarchical tag, like `State.NPC.Bob.Met`.
///
/// Stored as a [`SmolStr`] for cheap cloning. Construction validates the
/// format (non-empty, segments match `[A-Z][a-zA-Z0-9_]*`, separated by `.`).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(SmolStr);

impl Tag {
    /// Construct from a string, validating format.
    ///
    /// # Errors
    ///
    /// Returns [`adventure_core::Error::Asset`] if the tag is malformed
    /// (empty segment, invalid characters).
    pub fn new(s: impl Into<SmolStr>) -> Result<Self, adventure_core::Error> {
        let s: SmolStr = s.into();
        validate_tag(s.as_str())?;
        Ok(Self(s))
    }

    /// Construct without validation. Caller must ensure well-formedness.
    ///
    /// Useful for constants. Avoid in code that handles untrusted input.
    pub const fn from_static(s: &'static str) -> Self {
        // SmolStr's const constructor is `new_inline` — but we want compile-time
        // validation eventually. For now, accept the caller's promise.
        // SAFETY: just a transmute; SmolStr stores small strings inline.
        Self(SmolStr::new_inline(s))
    }

    /// The raw tag string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The raw SmolStr (for cheap clone / compare).
    pub fn as_smol(&self) -> &SmolStr {
        &self.0
    }

    /// Top-level segment (before first `.`), e.g. `State` for `State.NPC.Bob.Met`.
    pub fn root(&self) -> &str {
        match self.0.find('.') {
            Some(i) => &self.0[..i],
            None => self.0.as_str(),
        }
    }

    /// Whether this tag is a child (or equal to) `parent`.
    ///
    /// `State.NPC.Bob.Met` is child of `State.NPC.Bob`, `State.NPC`, `State`.
    pub fn is_child_of(&self, parent: &Tag) -> bool {
        let me = self.0.as_str();
        let p = parent.0.as_str();
        me == p || me.starts_with(&format!("{p}."))
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag({:?})", self.0)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl Serialize for Tag {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(self.0.as_str())
    }
}

impl<'de> Deserialize<'de> for Tag {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = SmolStr::deserialize(de)?;
        validate_tag(&s).map_err(serde::de::Error::custom)?;
        Ok(Self(s))
    }
}

impl PartialEq<str> for Tag {
    fn eq(&self, other: &str) -> bool {
        self.0.as_str() == other
    }
}

/// A set of tags, with hierarchical query support.
///
/// Mirrors UE's `FGameplayTagContainer`. Internally a `Vec<SmolStr>`-backed
/// `HashSet<Tag>`; size is bounded by the number of authored tags
/// (typically a few hundred).
#[derive(Clone, Default, Debug)]
pub struct Tags {
    set: std::collections::HashSet<Tag>,
}

impl Tags {
    /// Empty container.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a tag. No-op if already present.
    pub fn add(&mut self, tag: Tag) {
        self.set.insert(tag);
    }

    /// Remove a tag. No-op if not present.
    pub fn remove(&mut self, tag: &Tag) {
        self.set.remove(tag);
    }

    /// Exact membership (no hierarchy traversal).
    pub fn has(&self, tag: &Tag) -> bool {
        self.set.contains(tag)
    }

    /// True if any tag in the container matches or is a child of `parent`.
    ///
    /// `has_any(Tag::new("State.NPC").unwrap())` returns true if any
    /// `State.NPC.*` is set.
    pub fn has_any(&self, parent: &Tag) -> bool {
        self.set.iter().any(|t| t.is_child_of(parent))
    }

    /// True if every parent tag is matched (exact or by child).
    pub fn has_all(&self, parents: &[Tag]) -> bool {
        parents.iter().all(|p| self.has_any(p))
    }

    /// Iterate over all tags.
    pub fn iter(&self) -> impl Iterator<Item = &Tag> {
        self.set.iter()
    }

    /// Number of tags.
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Drain all tags.
    pub fn clear(&mut self) {
        self.set.clear();
    }

    /// Snapshot the tags as a sorted Vec (for serialization / save files).
    pub fn to_sorted_vec(&self) -> Vec<Tag> {
        let mut v: Vec<_> = self.set.iter().cloned().collect();
        v.sort();
        v
    }
}

impl IntoIterator for Tags {
    type Item = Tag;
    type IntoIter = std::collections::hash_set::IntoIter<Tag>;
    fn into_iter(self) -> Self::IntoIter {
        self.set.into_iter()
    }
}

impl Extend<Tag> for Tags {
    fn extend<I: IntoIterator<Item = Tag>>(&mut self, iter: I) {
        self.set.extend(iter);
    }
}

fn validate_tag(s: &str) -> Result<(), adventure_core::Error> {
    if s.is_empty() {
        return Err(adventure_core::Error::Asset(format!(
            "tag must not be empty"
        )));
    }
    for segment in s.split('.') {
        if segment.is_empty() {
            return Err(adventure_core::Error::Asset(format!(
                "tag segment must not be empty: {s:?}"
            )));
        }
        if !segment.chars().next().unwrap().is_ascii_uppercase() {
            return Err(adventure_core::Error::Asset(format!(
                "tag segment must start uppercase: {segment:?} in {s:?}"
            )));
        }
        for ch in segment.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '_' {
                return Err(adventure_core::Error::Asset(format!(
                    "tag segment has invalid char {ch:?} in {s:?}"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_construction_validates() {
        assert!(Tag::new("State.NPC.Bob.Met").is_ok());
        assert!(Tag::new("State").is_ok());
        assert!(Tag::new("").is_err());
        assert!(Tag::new("State..NPC").is_err());
        assert!(Tag::new("state.npc").is_err(), "lowercase rejected");
        assert!(Tag::new("State.NPC!").is_err(), "invalid char");
    }

    #[test]
    fn root_segment() {
        let t = Tag::new("State.NPC.Bob.Met").unwrap();
        assert_eq!(t.root(), "State");
    }

    #[test]
    fn is_child_of() {
        let child = Tag::new("State.NPC.Bob.Met").unwrap();
        assert!(child.is_child_of(&Tag::new("State").unwrap()));
        assert!(child.is_child_of(&Tag::new("State.NPC").unwrap()));
        assert!(child.is_child_of(&Tag::new("State.NPC.Bob").unwrap()));
        assert!(child.is_child_of(&Tag::new("State.NPC.Bob.Met").unwrap()));
        assert!(!child.is_child_of(&Tag::new("State.NPC.Alice").unwrap()));
    }

    #[test]
    fn tags_add_remove_has() {
        let mut t = Tags::new();
        assert!(!t.has(&Tag::new("State.X").unwrap()));
        t.add(Tag::new("State.NPC.Bob.Met").unwrap());
        assert!(t.has(&Tag::new("State.NPC.Bob.Met").unwrap()));
        t.remove(&Tag::new("State.NPC.Bob.Met").unwrap());
        assert!(!t.has(&Tag::new("State.NPC.Bob.Met").unwrap()));
    }

    #[test]
    fn tags_has_any_hierarchical() {
        let mut t = Tags::new();
        t.add(Tag::new("State.NPC.Bob.Met").unwrap());
        assert!(t.has_any(&Tag::new("State.NPC").unwrap()));
        assert!(t.has_any(&Tag::new("State").unwrap()));
        assert!(!t.has_any(&Tag::new("State.Door").unwrap()));
    }

    #[test]
    fn tags_has_all() {
        let mut t = Tags::new();
        t.add(Tag::new("State.NPC.Bob.Met").unwrap());
        t.add(Tag::new("State.Door.Cellar.Open").unwrap());
        let parents = vec![
            Tag::new("State.NPC").unwrap(),
            Tag::new("State.Door").unwrap(),
        ];
        assert!(t.has_all(&parents));
        let missing = vec![Tag::new("State.Quest").unwrap()];
        assert!(!t.has_all(&missing));
    }

    #[test]
    fn tag_serde_roundtrip() {
        let t = Tag::new("State.NPC.Bob.Met").unwrap();
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"State.NPC.Bob.Met\"");
        let back: Tag = serde_json::from_str(&json).unwrap();
        assert_eq!(back, t);
    }

    #[test]
    fn tags_to_sorted_vec() {
        let mut t = Tags::new();
        t.add(Tag::new("State.Zebra").unwrap());
        t.add(Tag::new("State.Apple").unwrap());
        t.add(Tag::new("State.Mango").unwrap());
        let v = t.to_sorted_vec();
        let names: Vec<&str> = v.iter().map(|t| t.as_str()).collect();
        assert_eq!(names, vec!["State.Apple", "State.Mango", "State.Zebra"]);
    }
}
