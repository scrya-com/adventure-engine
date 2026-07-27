//! Hierarchical state machine.
//!
//! Reference: UE's [`StateTree`](Plugins/Runtime/StateTree/) —
//! hierarchical state machine with enter/exit/tick callbacks.
//!
//! Phase 1 ships a minimal flat FSM; hierarchical composition lands in
//! Phase 5 when dialog/cutscene systems need it.

use adventure_core::SmolStr;
use serde::{Deserialize, Serialize};

/// A state identifier within a state machine. Interned string.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateId(pub SmolStr);

impl StateId {
    /// Construct from anything string-like.
    pub fn new(s: impl Into<SmolStr>) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// A minimal flat finite state machine.
///
/// Tracks `current_state` and `previous_state`. Transitions are explicit
/// via [`StateMachine::transition`]. Callers can read the current state
/// to drive their own tick logic.
///
/// Hierarchical / enter-exit-callback support is deferred to Phase 5.
#[derive(Clone, Debug, Default)]
pub struct StateMachine {
    current: Option<StateId>,
    previous: Option<StateId>,
}

impl StateMachine {
    /// Create with no current state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with an initial state.
    pub fn with_initial(initial: StateId) -> Self {
        Self {
            current: Some(initial),
            previous: None,
        }
    }

    /// Current state, if any.
    pub fn current(&self) -> Option<&StateId> {
        self.current.as_ref()
    }

    /// Previous state, if any.
    pub fn previous(&self) -> Option<&StateId> {
        self.previous.as_ref()
    }

    /// Transition to a new state. No-op if already there.
    ///
    /// Returns `true` if a transition occurred.
    pub fn transition(&mut self, to: StateId) -> bool {
        if self.current.as_ref() == Some(&to) {
            return false;
        }
        self.previous = self.current.take();
        self.current = Some(to);
        true
    }

    /// Whether we just transitioned from `previous` to `current` on the
    /// last call to [`transition`](Self::transition).
    pub fn just_changed(&self) -> bool {
        self.previous.is_some() && self.current.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state() {
        let sm = StateMachine::with_initial(StateId::new("idle"));
        assert_eq!(sm.current().unwrap().as_str(), "idle");
        assert!(sm.previous().is_none());
    }

    #[test]
    fn transition_records_previous() {
        let mut sm = StateMachine::with_initial(StateId::new("idle"));
        assert!(sm.transition(StateId::new("walking")));
        assert_eq!(sm.current().unwrap().as_str(), "walking");
        assert_eq!(sm.previous().unwrap().as_str(), "idle");
        assert!(sm.just_changed());
    }

    #[test]
    fn transition_to_same_is_noop() {
        let mut sm = StateMachine::with_initial(StateId::new("idle"));
        assert!(!sm.transition(StateId::new("idle")));
    }

    #[test]
    fn serde_state_id() {
        let id = StateId::new("chapter_2");
        let json = serde_json::to_string(&id).unwrap();
        let back: StateId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }
}
