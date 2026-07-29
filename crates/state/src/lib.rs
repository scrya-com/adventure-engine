//! Game state: tags, variables, state machines.
//!
//! Mirrors UE's [`FGameplayTagContainer`]
//! (GameplayTags/Classes/GameplayTagContainer.h:41) — hierarchical
//! dot-separated strings (e.g. `State.NPC.Bob.Met`) with query methods.
//!
//! Also includes [`VarTable`] — a typed key→value table for game variables
//! (integers, floats, strings, bools, asset references) that Rhai scripts
//! can read and write.
//!
//! Shared authored flag path constants live in [`flag_paths`] (e.g. Shawshank
//! PAC spine) so hosts, dialog side-effects, and headless walkthroughs use
//! one vocabulary.

#![deny(missing_docs)]

pub mod flag_paths;
pub mod tags;
pub mod var_table;
pub mod state_machine;

pub use state_machine::StateMachine;
pub use tags::{Tag, Tags};
pub use var_table::{VarTable, VarValue};
