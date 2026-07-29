//! Item combine rules — use item A on item B.
//!
//! Fail closed: unknown pairs produce [`CombineResult::Failed`] with a
//! default or author-supplied message. Successful recipes may consume
//! one or both inputs and optionally grant a result item.

use adventure_core::SmolStr;
use serde::{Deserialize, Serialize};

use crate::error::InventoryError;
use crate::inventory::Inventory;
use crate::item::Item;

/// Default fail-closed message when no recipe matches.
pub const DEFAULT_FAIL_MESSAGE: &str = "That doesn't work.";

/// One authored combine recipe.
///
/// Order of `a` / `b` is **insignificant** at match time (A on B ≡ B on A)
/// unless [`CombineRule::ordered`] is true.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CombineRule {
    /// First ingredient id.
    pub a: SmolStr,
    /// Second ingredient id.
    pub b: SmolStr,
    /// Result item id granted on success (optional — pure consume / message).
    #[serde(default)]
    pub result: Option<SmolStr>,
    /// Remove one of `a` from inventory.
    #[serde(default = "default_true")]
    pub consume_a: bool,
    /// Remove one of `b` from inventory.
    #[serde(default = "default_true")]
    pub consume_b: bool,
    /// Success message for UI / log.
    pub message: SmolStr,
    /// When true, only (`a` used on `b`) matches — not the reverse.
    #[serde(default)]
    pub ordered: bool,
    /// Optional named action id for scripting hooks.
    #[serde(default)]
    pub action: Option<SmolStr>,
}

fn default_true() -> bool {
    true
}

impl CombineRule {
    /// Build an unordered recipe that consumes both and grants `result`.
    pub fn recipe(
        a: impl Into<SmolStr>,
        b: impl Into<SmolStr>,
        result: impl Into<SmolStr>,
        message: impl Into<SmolStr>,
    ) -> Self {
        Self {
            a: a.into(),
            b: b.into(),
            result: Some(result.into()),
            consume_a: true,
            consume_b: true,
            message: message.into(),
            ordered: false,
            action: None,
        }
    }

    /// Whether (`used`, `target`) matches this rule.
    pub fn matches(&self, used: &str, target: &str) -> bool {
        if self.ordered {
            self.a.as_str() == used && self.b.as_str() == target
        } else {
            (self.a.as_str() == used && self.b.as_str() == target)
                || (self.a.as_str() == target && self.b.as_str() == used)
        }
    }
}

/// Outcome of attempting a combine (always fail-closed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CombineResult {
    /// Recipe matched and inventory was updated.
    Success {
        /// Human-readable success text.
        message: SmolStr,
        /// New item granted, if any.
        result: Option<SmolStr>,
        /// Optional action id for host scripting.
        action: Option<SmolStr>,
    },
    /// No recipe / cannot combine.
    Failed {
        /// Fail message (never panics — host shows this).
        message: SmolStr,
    },
}

impl CombineResult {
    /// Success?
    pub fn is_success(&self) -> bool {
        matches!(self, CombineResult::Success { .. })
    }

    /// Message for either arm.
    pub fn message(&self) -> &str {
        match self {
            CombineResult::Success { message, .. } | CombineResult::Failed { message } => {
                message.as_str()
            }
        }
    }
}

/// Table of combine recipes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CombineTable {
    /// Recipes in author order (first match wins).
    #[serde(default)]
    pub rules: Vec<CombineRule>,
    /// Override for fail-closed message.
    #[serde(default)]
    pub fail_message: Option<SmolStr>,
}

impl CombineTable {
    /// Empty table (everything fails closed).
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style rule insert.
    pub fn with_rule(mut self, rule: CombineRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Parse RON (bare `( rules: [...], ... )` or named newtype).
    ///
    /// # Errors
    ///
    /// [`InventoryError::Ron`] on parse failure.
    pub fn from_ron(s: &str) -> Result<Self, InventoryError> {
        if let Ok(t) = ron::from_str::<CombineTable>(s) {
            return Ok(t);
        }
        #[derive(Deserialize)]
        #[serde(rename = "CombineTable")]
        struct Wrapper(CombineTable);
        ron::from_str::<Wrapper>(s)
            .map(|w| w.0)
            .map_err(|e| InventoryError::Ron(e.to_string()))
    }

    /// Pretty RON serialize.
    ///
    /// # Errors
    ///
    /// [`InventoryError::Serialize`] on failure.
    pub fn to_ron(&self) -> Result<String, InventoryError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| InventoryError::Serialize(e.to_string()))
    }

    /// Find the first matching rule for (`used`, `target`).
    pub fn find(&self, used: &str, target: &str) -> Option<&CombineRule> {
        self.rules.iter().find(|r| r.matches(used, target))
    }

    /// Attempt to combine without mutating inventory (dry run).
    pub fn try_combine(&self, used: &str, target: &str) -> CombineResult {
        match self.find(used, target) {
            Some(rule) => CombineResult::Success {
                message: rule.message.clone(),
                result: rule.result.clone(),
                action: rule.action.clone(),
            },
            None => CombineResult::Failed {
                message: self
                    .fail_message
                    .clone()
                    .unwrap_or_else(|| DEFAULT_FAIL_MESSAGE.into()),
            },
        }
    }

    /// Apply a combine: check both items present, mutate inventory, grant result.
    ///
    /// `catalog` resolves result item definitions for stack rules. When the
    /// result id is unknown, it is still added as a non-stackable raw id.
    ///
    /// # Errors
    ///
    /// - [`InventoryError::NotEnough`] if either ingredient is missing
    /// - [`InventoryError::Full`] if granting the result needs a free slot
    ///
    /// Unknown recipes return `Ok(CombineResult::Failed)` (fail closed, not an error).
    pub fn apply(
        &self,
        inv: &mut Inventory,
        used: &str,
        target: &str,
        catalog: &ItemCatalog,
    ) -> Result<CombineResult, InventoryError> {
        let Some(rule) = self.find(used, target) else {
            return Ok(CombineResult::Failed {
                message: self
                    .fail_message
                    .clone()
                    .unwrap_or_else(|| DEFAULT_FAIL_MESSAGE.into()),
            });
        };

        // Ensure we hold both ingredients (same item twice needs count ≥ 2).
        if used == target {
            if !inv.has(used, 2) {
                return Err(InventoryError::NotEnough {
                    id: used.into(),
                    have: inv.count(used),
                    need: 2,
                });
            }
        } else {
            if !inv.has_any(used) {
                return Err(InventoryError::NotEnough {
                    id: used.into(),
                    have: 0,
                    need: 1,
                });
            }
            if !inv.has_any(target) {
                return Err(InventoryError::NotEnough {
                    id: target.into(),
                    have: 0,
                    need: 1,
                });
            }
        }

        // Map consume flags onto used/target (rule stores a/b, which may be swapped).
        let (consume_used, consume_target) = if rule.a.as_str() == used && rule.b.as_str() == target
        {
            (rule.consume_a, rule.consume_b)
        } else if rule.a.as_str() == target && rule.b.as_str() == used {
            (rule.consume_b, rule.consume_a)
        } else {
            // ordered mismatch shouldn't happen after find()
            (rule.consume_a, rule.consume_b)
        };

        if consume_used {
            inv.remove_one(used)?;
        }
        if consume_target {
            inv.remove_one(target)?;
        }

        if let Some(ref result_id) = rule.result {
            if let Some(def) = catalog.get(result_id.as_str()) {
                inv.add(def)?;
            } else {
                inv.add_id(result_id.as_str())?;
            }
        }

        Ok(CombineResult::Success {
            message: rule.message.clone(),
            result: rule.result.clone(),
            action: rule.action.clone(),
        })
    }
}

/// Lookup table of item definitions by id.
#[derive(Clone, Debug, Default)]
pub struct ItemCatalog {
    items: std::collections::BTreeMap<SmolStr, Item>,
}

impl ItemCatalog {
    /// Empty catalog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert / replace an item definition.
    pub fn insert(&mut self, item: Item) {
        self.items.insert(item.id.clone(), item);
    }

    /// Builder-style insert.
    pub fn with(mut self, item: Item) -> Self {
        self.insert(item);
        self
    }

    /// Look up by id.
    pub fn get(&self, id: &str) -> Option<&Item> {
        self.items.get(id)
    }

    /// Whether the catalog knows `id`.
    pub fn contains(&self, id: &str) -> bool {
        self.items.contains_key(id)
    }

    /// All items.
    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        self.items.values()
    }

    /// Number of definitions.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Empty?
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oil_lamp_table() -> (CombineTable, ItemCatalog, Inventory) {
        let oil = Item::new("oil", "Oil", "A flask of lamp oil.");
        let lamp = Item::new("lamp", "Lamp", "An empty oil lamp.");
        let lit = Item::new("lit_lamp", "Lit Lamp", "A warm, glowing lamp.");
        let catalog = ItemCatalog::new()
            .with(oil.clone())
            .with(lamp.clone())
            .with(lit);
        let table = CombineTable::new().with_rule(CombineRule::recipe(
            "oil",
            "lamp",
            "lit_lamp",
            "You fill the lamp with oil and light it.",
        ));
        let mut inv = Inventory::new();
        inv.add(&oil).unwrap();
        inv.add(&lamp).unwrap();
        (table, catalog, inv)
    }

    #[test]
    fn success_unordered() {
        let (table, catalog, mut inv) = oil_lamp_table();
        // Reverse order still works
        let r = table
            .apply(&mut inv, "lamp", "oil", &catalog)
            .unwrap();
        assert!(r.is_success());
        assert!(!inv.has_any("oil"));
        assert!(!inv.has_any("lamp"));
        assert!(inv.has_any("lit_lamp"));
        assert_eq!(
            r.message(),
            "You fill the lamp with oil and light it."
        );
    }

    #[test]
    fn fail_closed() {
        let (table, catalog, mut inv) = oil_lamp_table();
        inv.add_id("rock").unwrap();
        let r = table.apply(&mut inv, "rock", "oil", &catalog).unwrap();
        assert!(!r.is_success());
        assert_eq!(r.message(), DEFAULT_FAIL_MESSAGE);
        // Inventory unchanged for failed combine
        assert!(inv.has_any("rock"));
        assert!(inv.has_any("oil"));
    }

    #[test]
    fn ordered_rule() {
        let table = CombineTable::new().with_rule(CombineRule {
            a: "key".into(),
            b: "lock".into(),
            result: None,
            consume_a: true,
            consume_b: false,
            message: "Unlocked.".into(),
            ordered: true,
            action: Some("unlock".into()),
        });
        assert!(table.find("key", "lock").is_some());
        assert!(table.find("lock", "key").is_none());
    }

    #[test]
    fn ron_round_trip() {
        let table = CombineTable::new()
            .with_rule(CombineRule::recipe("oil", "lamp", "lit_lamp", "Lit!"));
        let s = table.to_ron().unwrap();
        let back = CombineTable::from_ron(&s).unwrap();
        assert_eq!(table, back);
    }

    #[test]
    fn missing_ingredient_errors() {
        let (table, catalog, mut inv) = oil_lamp_table();
        inv.remove_one("oil").unwrap();
        let err = table.apply(&mut inv, "oil", "lamp", &catalog).unwrap_err();
        assert!(matches!(err, InventoryError::NotEnough { .. }));
    }
}
