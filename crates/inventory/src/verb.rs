//! Player-facing verb vocabulary for inventory + world interaction.
//!
//! Extends the locomotion 3-verb set (Look / Use / Talk-to) with
//! Pickup and Give for inventory-centric point-and-click.

use serde::{Deserialize, Serialize};

/// Verb kinds shown on the verb coin and authored on items / hotspots.
///
/// Matches the modern Thimbleweed-style coin:
/// Look · Use · Talk · Pickup · Give.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum VerbKind {
    /// Examine / look at.
    Look,
    /// Use in place, or use-with when a target is selected.
    Use,
    /// Speak to an NPC.
    Talk,
    /// Pick up into inventory.
    Pickup,
    /// Give inventory item to an NPC / hotspot.
    Give,
    /// Explicit item-on-target binding (authored on items as `UseOn`).
    UseOn,
}

impl VerbKind {
    /// Default verbs for the radial coin (order = sector order, clockwise from top).
    pub const COIN_DEFAULT: [VerbKind; 4] = [
        VerbKind::Look,
        VerbKind::Use,
        VerbKind::Talk,
        VerbKind::Pickup,
    ];

    /// Short UI label.
    pub fn label(self) -> &'static str {
        match self {
            VerbKind::Look => "Look",
            VerbKind::Use => "Use",
            VerbKind::Talk => "Talk",
            VerbKind::Pickup => "Pickup",
            VerbKind::Give => "Give",
            VerbKind::UseOn => "Use on",
        }
    }

    /// All standard coin verbs including Give (5-sector coin).
    pub const COIN_WITH_GIVE: [VerbKind; 5] = [
        VerbKind::Look,
        VerbKind::Use,
        VerbKind::Talk,
        VerbKind::Pickup,
        VerbKind::Give,
    ];
}

impl std::fmt::Display for VerbKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_stable() {
        assert_eq!(VerbKind::Look.label(), "Look");
        assert_eq!(VerbKind::Pickup.label(), "Pickup");
        assert_eq!(VerbKind::UseOn.label(), "Use on");
    }

    #[test]
    fn ron_pascal_case() {
        let v = VerbKind::Pickup;
        let s = ron::to_string(&v).unwrap();
        assert_eq!(s, "Pickup");
        let back: VerbKind = ron::from_str(&s).unwrap();
        assert_eq!(back, VerbKind::Pickup);
    }
}
