//! Player inventory bag: add / remove / has / count with optional capacity.

use adventure_core::{AssetId, SmolStr};
use serde::{Deserialize, Serialize};

use crate::error::InventoryError;
use crate::item::Item;

/// One slot: item id + stack count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slot {
    /// Item definition id.
    pub id: SmolStr,
    /// Stack count (≥ 1).
    pub count: u32,
}

impl Slot {
    /// Soft asset id (`items/<id>`).
    pub fn asset_id(&self) -> AssetId {
        AssetId::from_path(&format!("items/{}", self.id))
    }
}

/// Ordered bag of item stacks.
///
/// Distinct item ids each occupy one slot. When an item is stackable,
/// counts merge into the existing slot up to `max_stack`. Capacity
/// (if set) limits the number of **distinct slots**, not total items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Inventory {
    /// Slots in insertion order.
    slots: Vec<Slot>,
    /// Optional max distinct slots.
    #[serde(default)]
    capacity: Option<usize>,
}

impl Inventory {
    /// Empty inventory with no capacity limit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Empty inventory with a hard slot capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::new(),
            capacity: Some(capacity),
        }
    }

    /// Slot capacity, if limited.
    pub fn capacity(&self) -> Option<usize> {
        self.capacity
    }

    /// Number of distinct slots occupied.
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the bag is empty.
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    /// Iterate slots in insertion order.
    pub fn slots(&self) -> impl Iterator<Item = &Slot> {
        self.slots.iter()
    }

    /// Item ids for save serialization.
    ///
    /// Stacks expand to repeated asset ids when count > 1 (save format is
    /// a flat `Vec<AssetId>`).
    pub fn to_asset_ids(&self) -> Vec<AssetId> {
        let mut out = Vec::new();
        for s in &self.slots {
            let aid = s.asset_id();
            for _ in 0..s.count {
                out.push(aid);
            }
        }
        out
    }

    /// Rebuild from a flat asset-id list (save load).
    ///
    /// Expects ids of the form `items/<id>` produced by [`Slot::asset_id`].
    /// Unknown path shapes are kept as the raw path string without the
    /// `items/` prefix when present.
    pub fn from_asset_ids(ids: &[AssetId], resolve: impl Fn(AssetId) -> Option<SmolStr>) -> Self {
        let mut inv = Self::new();
        for &aid in ids {
            if let Some(id) = resolve(aid) {
                // stackable path: ignore catalog max here; caller can validate
                let _ = inv.add_raw(&id, 1, u32::MAX, true);
            }
        }
        inv
    }

    /// Count of a given item id (0 if absent).
    pub fn count(&self, id: &str) -> u32 {
        self.slots
            .iter()
            .find(|s| s.id.as_str() == id)
            .map(|s| s.count)
            .unwrap_or(0)
    }

    /// Whether at least `n` of `id` are held.
    pub fn has(&self, id: &str, n: u32) -> bool {
        self.count(id) >= n
    }

    /// Whether any of `id` is held.
    pub fn has_any(&self, id: &str) -> bool {
        self.has(id, 1)
    }

    /// Add one copy using item definition for stack rules.
    ///
    /// # Errors
    ///
    /// [`InventoryError::Full`] when a new slot would be needed past capacity.
    pub fn add(&mut self, item: &Item) -> Result<(), InventoryError> {
        self.add_raw(
            item.id.as_str(),
            1,
            item.effective_max_stack(),
            item.stackable,
        )
    }

    /// Add `amount` copies of `id`.
    ///
    /// # Errors
    ///
    /// [`InventoryError::Full`] when a new slot is required past capacity.
    pub fn add_n(
        &mut self,
        item: &Item,
        amount: u32,
    ) -> Result<(), InventoryError> {
        if amount == 0 {
            return Ok(());
        }
        self.add_raw(
            item.id.as_str(),
            amount,
            item.effective_max_stack(),
            item.stackable,
        )
    }

    /// Add by raw id (when catalog is not at hand). Non-stackable.
    ///
    /// # Errors
    ///
    /// [`InventoryError::Full`] when capacity is exceeded.
    pub fn add_id(&mut self, id: impl Into<SmolStr>) -> Result<(), InventoryError> {
        let id = id.into();
        self.add_raw(id.as_str(), 1, 1, false)
    }

    fn add_raw(
        &mut self,
        id: &str,
        amount: u32,
        max_stack: u32,
        stackable: bool,
    ) -> Result<(), InventoryError> {
        if amount == 0 {
            return Ok(());
        }
        if let Some(slot) = self.slots.iter_mut().find(|s| s.id.as_str() == id) {
            if stackable {
                slot.count = slot.count.saturating_add(amount).min(max_stack.max(1));
            } else {
                // Non-stackable: already holding one — ignore extra (or treat as full of that item)
                // Keep count at 1.
                slot.count = 1;
            }
            return Ok(());
        }
        // Need a new slot
        if let Some(cap) = self.capacity {
            if self.slots.len() >= cap {
                return Err(InventoryError::Full { capacity: cap });
            }
        }
        let count = if stackable {
            amount.min(max_stack.max(1))
        } else {
            1
        };
        self.slots.push(Slot {
            id: id.into(),
            count,
        });
        Ok(())
    }

    /// Remove `amount` of `id`.
    ///
    /// # Errors
    ///
    /// [`InventoryError::NotEnough`] if fewer than `amount` are held.
    pub fn remove(&mut self, id: &str, amount: u32) -> Result<(), InventoryError> {
        if amount == 0 {
            return Ok(());
        }
        let Some(idx) = self.slots.iter().position(|s| s.id.as_str() == id) else {
            return Err(InventoryError::NotEnough {
                id: id.into(),
                have: 0,
                need: amount,
            });
        };
        let have = self.slots[idx].count;
        if have < amount {
            return Err(InventoryError::NotEnough {
                id: id.into(),
                have,
                need: amount,
            });
        }
        if have == amount {
            self.slots.remove(idx);
        } else {
            self.slots[idx].count = have - amount;
        }
        Ok(())
    }

    /// Remove one of `id`.
    ///
    /// # Errors
    ///
    /// [`InventoryError::NotEnough`] if none held.
    pub fn remove_one(&mut self, id: &str) -> Result<(), InventoryError> {
        self.remove(id, 1)
    }

    /// Clear all slots.
    pub fn clear(&mut self) {
        self.slots.clear();
    }

    /// Slot index for UI hit-testing, if present.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.slots.iter().position(|s| s.id.as_str() == id)
    }

    /// Get slot by index (inventory bar order).
    pub fn get(&self, index: usize) -> Option<&Slot> {
        self.slots.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rock() -> Item {
        Item::new("rock", "Rock", "A rock.")
    }

    fn coin() -> Item {
        let mut i = Item::new("coin", "Coin", "Shiny.");
        i.stackable = true;
        i.max_stack = 99;
        i
    }

    #[test]
    fn add_has_remove() {
        let mut inv = Inventory::new();
        inv.add(&rock()).unwrap();
        assert!(inv.has_any("rock"));
        assert_eq!(inv.count("rock"), 1);
        inv.remove_one("rock").unwrap();
        assert!(!inv.has_any("rock"));
        assert!(inv.is_empty());
    }

    #[test]
    fn capacity_enforced() {
        let mut inv = Inventory::with_capacity(1);
        inv.add(&rock()).unwrap();
        let mut key = Item::new("key", "Key", "Key.");
        key.id = "key".into();
        assert!(matches!(
            inv.add(&key),
            Err(InventoryError::Full { capacity: 1 })
        ));
    }

    #[test]
    fn stackable_merges() {
        let mut inv = Inventory::new();
        let c = coin();
        inv.add_n(&c, 3).unwrap();
        inv.add_n(&c, 2).unwrap();
        assert_eq!(inv.len(), 1);
        assert_eq!(inv.count("coin"), 5);
        inv.remove("coin", 4).unwrap();
        assert_eq!(inv.count("coin"), 1);
    }

    #[test]
    fn remove_not_enough() {
        let mut inv = Inventory::new();
        inv.add(&rock()).unwrap();
        assert!(matches!(
            inv.remove("rock", 2),
            Err(InventoryError::NotEnough { have: 1, need: 2, .. })
        ));
    }

    #[test]
    fn to_asset_ids_expands_stacks() {
        let mut inv = Inventory::new();
        inv.add_n(&coin(), 3).unwrap();
        let ids = inv.to_asset_ids();
        assert_eq!(ids.len(), 3);
        assert!(ids.iter().all(|a| *a == AssetId::from_path("items/coin")));
    }

    #[test]
    fn ron_round_trip() {
        let mut inv = Inventory::with_capacity(8);
        inv.add(&rock()).unwrap();
        inv.add_n(&coin(), 2).unwrap();
        let s = ron::ser::to_string_pretty(&inv, ron::ser::PrettyConfig::default()).unwrap();
        let back: Inventory = ron::from_str(&s).unwrap();
        assert_eq!(inv, back);
    }
}
