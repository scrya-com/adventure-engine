//! Game state: tags, variables, state machines.
//!
//! Mirrors UE's [`FGameplayTagContainer`]
//! (GameplayTags/Classes/GameplayTagContainer.h:41) — hierarchical
//! dot-separated strings (e.g. `State.NPC.Bob.Met`) with query methods.
//!
//! Also includes [`VarTable`] — a typed key→value table for game variables
//! (integers, floats, strings, bools, asset references) that Rhai scripts
//! can read and write.

#![deny(missing_docs)]

pub mod tags;
pub mod var_table;
pub mod state_machine;

pub use state_machine::StateMachine;
pub use tags::{Tag, Tags};
pub use var_table::{VarTable, VarValue};
