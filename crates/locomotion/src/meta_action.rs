//! Meta Action vocabulary — mirrors Dart `meta_action.dart`.

use serde::{Deserialize, Serialize};

/// High-level, generation-agnostic character intents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetaAction {
    /// Idle / rest.
    Idle,
    /// Locomotion.
    Walk,
    /// Sit.
    Sit,
    /// Stand from sit.
    Stand,
    /// Examine / look.
    Examine,
    /// Pick up item.
    PickUp,
    /// Put down item.
    PutDown,
    /// Use in place.
    Use,
    /// Happy reaction.
    ReactHappy,
    /// Surprised reaction.
    ReactSurprised,
    /// Talk.
    Talk,
}

/// snake_case token used for AVAL state matching (Dart `MetaActionToken.token`).
pub fn meta_action_token(action: MetaAction) -> &'static str {
    match action {
        MetaAction::Idle => "idle",
        MetaAction::Walk => "walk",
        MetaAction::Sit => "sit",
        MetaAction::Stand => "stand",
        MetaAction::Examine => "examine",
        MetaAction::PickUp => "pick_up",
        MetaAction::PutDown => "put_down",
        MetaAction::Use => "use",
        MetaAction::ReactHappy => "react_happy",
        MetaAction::ReactSurprised => "react_surprised",
        MetaAction::Talk => "talk",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_match_dart() {
        assert_eq!(meta_action_token(MetaAction::PickUp), "pick_up");
        assert_eq!(meta_action_token(MetaAction::ReactHappy), "react_happy");
        assert_eq!(meta_action_token(MetaAction::Talk), "talk");
        assert_eq!(meta_action_token(MetaAction::Examine), "examine");
    }
}
