//! Inventory: items, bag, combine rules, verb coin.
//!
//! Items are authored as RON files (`assets/items/<name>.item.ron`) and
//! combined via a fail-closed recipe table. The verb coin is pure layout
//! math for Look / Use / Talk / Pickup / Give. See `docs/DATA-FORMATS.md`.
//!
//! # Modules
//!
//! - [`item`] — [`Item`] + [`ItemVerb`] definitions
//! - [`inventory`] — [`Inventory`] bag (add/remove/has/count)
//! - [`combine`] — [`CombineTable`] recipes + [`ItemCatalog`]
//! - [`verb`] — [`VerbKind`] enum
//! - [`coin`] — [`VerbCoin`] radial hit-test + [`InventoryBar`]
//!
//! # Example
//!
//! ```
//! use adventure_inventory::{CombineRule, CombineTable, Inventory, Item, ItemCatalog};
//!
//! let oil = Item::new("oil", "Oil", "Lamp oil.");
//! let lamp = Item::new("lamp", "Lamp", "Empty lamp.");
//! let lit = Item::new("lit_lamp", "Lit Lamp", "Glowing.");
//! let catalog = ItemCatalog::new().with(oil.clone()).with(lamp.clone()).with(lit);
//!
//! let mut inv = Inventory::new();
//! inv.add(&oil).unwrap();
//! inv.add(&lamp).unwrap();
//!
//! let table = CombineTable::new().with_rule(CombineRule::recipe(
//!     "oil", "lamp", "lit_lamp", "You light the lamp.",
//! ));
//! let result = table.apply(&mut inv, "oil", "lamp", &catalog).unwrap();
//! assert!(result.is_success());
//! assert!(inv.has_any("lit_lamp"));
//! ```

#![deny(missing_docs)]

pub mod coin;
pub mod combine;
pub mod error;
pub mod inventory;
pub mod item;
pub mod verb;

pub use coin::{InventoryBar, VerbCoin, VerbSector};
pub use combine::{
    CombineResult, CombineRule, CombineTable, ItemCatalog, DEFAULT_FAIL_MESSAGE,
};
pub use error::InventoryError;
pub use inventory::{Inventory, Slot};
pub use item::{Item, ItemVerb};
pub use verb::VerbKind;
