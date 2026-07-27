//! Y-down screen compass + AVAL walk state resolve.
//!
//! Mirrors Dart `ardy_motion.dart`:
//! - `compassFromScreenHeading`
//! - `compassLetterToWalkKey`
//! - `resolveAvalWalkFromScreenHeading`
//! - walk-ring key order used by plant / ring hops

use std::collections::HashSet;

/// 8-dir walk ring keys, clockwise from north (screen Y-down).
///
/// Matches `scene_stage._kWalkRingKeys`.
pub const WALK_RING_KEYS: [&str; 8] = [
    "north",
    "northeast",
    "east",
    "southeast",
    "south",
    "southwest",
    "west",
    "northwest",
];

/// Screen-space compass letter from heading (Y-down atan2, radians).
///
/// Bins: E=0°, SE=+45°, S=+90°, …, NE=−45°.
pub fn compass_from_screen_heading(screen_heading_rad: f64) -> &'static str {
    let mut deg = screen_heading_rad * 180.0 / std::f64::consts::PI;
    while deg > 180.0 {
        deg -= 360.0;
    }
    while deg <= -180.0 {
        deg += 360.0;
    }
    if deg > -22.5 && deg <= 22.5 {
        "E"
    } else if deg > 22.5 && deg <= 67.5 {
        "SE"
    } else if deg > 67.5 && deg <= 112.5 {
        "S"
    } else if deg > 112.5 && deg <= 157.5 {
        "SW"
    } else if deg > 157.5 || deg <= -157.5 {
        "W"
    } else if deg > -157.5 && deg <= -112.5 {
        "NW"
    } else if deg > -112.5 && deg <= -67.5 {
        "N"
    } else {
        "NE" // -67.5 .. -22.5
    }
}

/// Compass letter (`E`/`SE`/…) → isometric walk-key (`east`/`southeast`/…).
pub fn compass_letter_to_walk_key(letter: &str) -> &'static str {
    match letter.trim().to_ascii_uppercase().as_str() {
        "E" => "east",
        "NE" => "northeast",
        "N" => "north",
        "NW" => "northwest",
        "W" => "west",
        "SW" => "southwest",
        "S" => "south",
        "SE" => "southeast",
        _ => "south",
    }
}

/// Strip optional `walk_` prefix so `"walk_southeast"` and `"southeast"` match.
pub fn walk_ring_key(state: &str) -> &str {
    state.strip_prefix("walk_").unwrap_or(state)
}

/// Index of a walk ring key, or `None` if not in the 8-dir ring.
pub fn walk_ring_index(state: &str) -> Option<usize> {
    let key = walk_ring_key(state);
    WALK_RING_KEYS.iter().position(|&k| k == key)
}

/// Candidate `(state, mirror)` pairs for a walk compass key.
///
/// Prefer true `walk_{dir}` only; horizontal mirror is last-resort for
/// east/west-only packs.
fn aval_walk_candidates(compass: &str) -> &'static [(&'static str, bool)] {
    match compass {
        "east" => &[("walk_east", false), ("walk_west", true), ("walk", false)],
        "west" => &[("walk_west", false), ("walk_east", true), ("walk", false)],
        "north" => &[("walk_north", false), ("walk", false)],
        "south" => &[("walk_south", false), ("walk", false)],
        "northeast" => &[
            ("walk_northeast", false),
            ("walk_east", false),
            ("walk_north", false),
            ("walk", false),
        ],
        "northwest" => &[
            ("walk_northwest", false),
            ("walk_west", false),
            ("walk_north", false),
            ("walk", false),
        ],
        "southeast" => &[
            ("walk_southeast", false),
            ("walk_east", false),
            ("walk_south", false),
            ("walk", false),
        ],
        "southwest" => &[
            ("walk_southwest", false),
            ("walk_west", false),
            ("walk_south", false),
            ("walk", false),
        ],
        _ => &[],
    }
}

/// Resolved AVAL walk clip for a screen heading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvalWalkResolve {
    /// Chosen state id (e.g. `walk_east`).
    pub state: String,
    /// Whether host should billboard-flip (legacy east-only packs).
    pub mirror: bool,
    /// Walk compass key (`east`, `northwest`, …).
    pub compass: String,
    /// Letter bin (`E`, `NW`, …).
    pub letter: String,
}

/// Resolve AVAL walk state for a **Y-down screen** heading.
///
/// Returns `None` when [available] has no matching candidate.
pub fn resolve_aval_walk_from_screen_heading(
    screen_heading_rad: f64,
    available: &HashSet<String>,
) -> Option<AvalWalkResolve> {
    let letter = compass_from_screen_heading(screen_heading_rad);
    let compass = compass_letter_to_walk_key(letter);
    for &(candidate, mirror) in aval_walk_candidates(compass) {
        if available.contains(candidate) {
            return Some(AvalWalkResolve {
                state: candidate.to_string(),
                mirror,
                compass: compass.to_string(),
                letter: letter.to_string(),
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_bins_match_dart() {
        assert_eq!(compass_from_screen_heading(0.0), "E");
        assert_eq!(compass_from_screen_heading(1.5708), "S");
        assert_eq!(compass_from_screen_heading(3.1416), "W");
        assert_eq!(compass_from_screen_heading(-1.5708), "N");
        assert_eq!(
            compass_letter_to_walk_key(compass_from_screen_heading(0.0)),
            "east"
        );
        assert_eq!(
            compass_letter_to_walk_key(compass_from_screen_heading(1.5708)),
            "south"
        );
        assert_eq!(
            compass_letter_to_walk_key(compass_from_screen_heading(3.1416)),
            "west"
        );
        assert_eq!(
            compass_letter_to_walk_key(compass_from_screen_heading(-1.5708)),
            "north"
        );
    }

    #[test]
    fn resolve_prefers_true_walk_then_mirror() {
        let full: HashSet<String> = [
            "idle",
            "walk_east",
            "walk_west",
            "walk_north",
            "walk_south",
            "walk_northeast",
            "walk_northwest",
            "walk_southeast",
            "walk_southwest",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let e = resolve_aval_walk_from_screen_heading(0.0, &full).unwrap();
        assert_eq!(e.state, "walk_east");
        assert!(!e.mirror);
        assert_eq!(e.letter, "E");

        let only_west: HashSet<String> =
            ["walk_west", "idle"].into_iter().map(String::from).collect();
        let mirror = resolve_aval_walk_from_screen_heading(0.0, &only_west).unwrap();
        assert_eq!(mirror.state, "walk_west");
        assert!(mirror.mirror);

        let idle_only: HashSet<String> = ["idle"].into_iter().map(String::from).collect();
        assert!(resolve_aval_walk_from_screen_heading(0.0, &idle_only).is_none());
    }
}
