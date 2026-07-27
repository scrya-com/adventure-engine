//! Player-facing 3-verb interface — mirrors Dart `verb.dart`.

use serde::{Deserialize, Serialize};

use crate::meta_action::MetaAction;

/// Look / Use / Talk-to (Thimbleweed-style modern scheme).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verb {
    /// Inspect → always [MetaAction::Examine].
    Look,
    /// Contextual: pick_up if can_pick_up else use.
    Use,
    /// Speak → always [MetaAction::Talk].
    TalkTo,
}

impl Verb {
    /// Display label for UI strips.
    pub fn label(self) -> &'static str {
        match self {
            Verb::Look => "Look",
            Verb::Use => "Use",
            Verb::TalkTo => "Talk-to",
        }
    }

    /// Preferred [MetaAction] for this verb on an item (Dart `metaActionFor`).
    ///
    /// `can_pick_up` only affects [Verb::Use].
    pub fn meta_action_for(self, can_pick_up: bool) -> MetaAction {
        match self {
            Verb::Look => MetaAction::Examine,
            Verb::TalkTo => MetaAction::Talk,
            Verb::Use => {
                if can_pick_up {
                    MetaAction::PickUp
                } else {
                    MetaAction::Use
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_maps_pickup_when_takeable() {
        assert_eq!(Verb::Use.meta_action_for(true), MetaAction::PickUp);
        assert_eq!(Verb::Use.meta_action_for(false), MetaAction::Use);
    }

    #[test]
    fn look_and_talk_fixed() {
        assert_eq!(Verb::Look.meta_action_for(true), MetaAction::Examine);
        assert_eq!(Verb::TalkTo.meta_action_for(false), MetaAction::Talk);
    }
}
