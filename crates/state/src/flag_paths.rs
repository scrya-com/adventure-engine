//! Canonical hierarchical tag paths for authored game flags.
//!
//! These are string constants (not pre-built [`Tag`]s) so hosts can pass them
//! through [`Tag::new`] without pulling game-specific content into the tag
//! type system. Demo hosts (Shawshank PAC) and save schemas share the same
//! paths so Rhai/`set_flag` names stay aligned with HTML smoke.

/// Shawshank Cell Block C MVP spine — six host flags.
///
/// Ordering (happy path):
/// `examined_cell` → `andy_arrived` → `noticed_andy` / `can_talk_andy`
/// → `met_andy` / `knows_red_business` → contraband hotspot revealed.
pub mod shawshank {
    /// Player examined Red's cell (`examined_cell` in NCP JSON).
    pub const EXAMINED_CELL: &str = "State.Flag.ExaminedCell";
    /// Next-day bus arrived; Andy's cell occupied (`andy_arrived`).
    pub const ANDY_ARRIVED: &str = "State.Flag.AndyArrived";
    /// Player looked at occupied Andy cell (`noticed_andy`).
    pub const NOTICED_ANDY: &str = "State.Flag.NoticedAndy";
    /// Talk verb unlocked (host also derives from [`NOTICED_ANDY`]).
    pub const CAN_TALK_ANDY: &str = "State.Flag.CanTalkAndy";
    /// First-meeting dialog completed (`met_andy`).
    pub const MET_ANDY: &str = "State.Flag.MetAndy";
    /// Knows Red's contraband business; reveals loose stone
    /// (`knows_red_business`).
    pub const KNOWS_RED_BUSINESS: &str = "State.Flag.KnowsRedBusiness";

    /// All six flag paths in spine order (for tests / debug HUD).
    pub const ALL: &[&str] = &[
        EXAMINED_CELL,
        ANDY_ARRIVED,
        NOTICED_ANDY,
        CAN_TALK_ANDY,
        MET_ANDY,
        KNOWS_RED_BUSINESS,
    ];
}

#[cfg(test)]
mod tests {
    use super::shawshank;
    use crate::Tag;

    #[test]
    fn shawshank_paths_are_valid_tags() {
        assert_eq!(shawshank::ALL.len(), 6);
        for path in shawshank::ALL {
            Tag::new(*path).unwrap_or_else(|e| panic!("invalid {path}: {e}"));
        }
    }
}
