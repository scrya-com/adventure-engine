//! Plant controller: turn-before-plant + ARDY-style stride timing.
//!
//! Pure extract of `scene_stage` locomotion plant rules:
//! - `_walkRingHops` / `requireVisual` settle gate
//! - `_avalTripTiming` (strideNorm 0.10, cadence 2 Hz)
//! - `gaitLockedPlantT` foot-down hold discretisation
//!
//! **Moonwalk contract:** when ring hops ≥ 2 and not from idle, plant progress
//! stays at 0 until the host reports `route_settled` (AVAL multi-hop drained
//! and target walk_* visual active). Planting NW while body is still on
//! walk_southeast mid-ring-turn is the SE→NW moonwalk bug. Adjacent 1-hop
//! facing changes soft-settle (hard-cut packs flip quickly).

use crate::compass::{walk_ring_index, WALK_RING_KEYS};

/// Norm plant travel per ARDY-style footfall (`_kAvalStepNormPerGait`).
pub const AVAL_STEP_NORM_PER_GAIT: f64 = 0.10;

/// Footfalls per second while plant advances (`_kAvalCadenceHz`).
pub const AVAL_CADENCE_HZ: f64 = 2.0;

/// Minimum wall-clock plant duration (seconds).
pub const AVAL_MIN_DURATION_SEC: f64 = 0.22;

/// Maximum wall-clock plant duration (seconds).
pub const AVAL_MAX_DURATION_SEC: f64 = 12.0;

/// Contact-phase hold fraction within each stride cycle (`gaitLockedPlantT`).
pub const GAIT_HOLD_FRAC: f64 = 0.22;

/// Trip timing from floor distance (ARDY plant cadence).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TripTiming {
    /// Wall-clock plant duration in seconds.
    pub duration_sec: f64,
    /// Footfall count for gait-locked plant quantisation (≥ 1).
    pub strides: u32,
}

/// Sample of plant progress for one host tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlantSample {
    /// Linear elapsed / duration in \[0, 1\] (0 while held pre-settle).
    pub t_linear: f64,
    /// Gait-locked plant phase in \[0, 1\] (0 while held pre-settle).
    pub t: f64,
    /// True when plant is frozen waiting for AVAL route drain.
    pub held: bool,
    /// True when linear progress has reached the end.
    pub finished: bool,
}

/// Min hops on the 8-dir walk ring between two `walk_*` states (0 = same).
///
/// Accepts bare keys (`southeast`) or full state ids (`walk_southeast`).
/// Unknown states: 0 if equal, else 1 (matches Dart `_walkRingHops`).
pub fn walk_ring_hops(from_state: Option<&str>, to_state: &str) -> u32 {
    let a = from_state.and_then(walk_ring_index);
    let b = walk_ring_index(to_state);
    match (a, b) {
        (Some(ai), Some(bi)) => {
            if ai == bi {
                return 0;
            }
            let n = WALK_RING_KEYS.len();
            let cw = (bi + n - ai) % n;
            let ccw = (ai + n - bi) % n;
            cw.min(ccw) as u32
        }
        _ => {
            if from_state == Some(to_state) {
                0
            } else {
                1
            }
        }
    }
}

/// True when visual is idle / unset (soft-settle path allowed).
pub fn is_from_idle(from_visual: Option<&str>) -> bool {
    match from_visual {
        None => true,
        Some(s) => {
            let t = s.trim();
            t.is_empty() || t == "idle" || t == "intro"
        }
    }
}

/// Hard settle required for multi-hop facing changes (not idle→walk).
///
/// `require_visual = hops >= 2 && !from_idle` — plant must not advance until
/// the host sets route settled (visual == target, not transitioning, multi-hop
/// drained). Soft settle for idle→walk, same-facing re-walk, **or adjacent
/// 1-hop** (E→SE): hard-cut packs flip in one cut; sticky hard-settle on every
/// small turn hurt smoothness more than it prevented moonwalk.
pub fn require_visual_settle(from_visual: Option<&str>, to_state: &str) -> bool {
    let hops = walk_ring_hops(from_visual, to_state);
    hops >= 2 && !is_from_idle(from_visual)
}

/// Soft settle timeout (ms) used by Dart when not hard-settling.
pub const SOFT_SETTLE_MS: u32 = 280;

/// Hard-settle wait budget: `4000 + hops * 1200` ms (Dart default).
pub fn hard_settle_timeout_ms(hops: u32) -> u32 {
    4000 + hops * 1200
}

/// Soft / first-entry settle wait budget: 6000 ms (Dart default).
pub const SOFT_SETTLE_TIMEOUT_MS: u32 = 6000;

/// Trip timing: strides = ceil(dist / 0.10), duration = clamp(strides/2, 0.22, 12).
pub fn aval_trip_timing(dist: f64) -> TripTiming {
    let strides = ((dist / AVAL_STEP_NORM_PER_GAIT).ceil() as i64).max(1) as u32;
    let duration_sec = (strides as f64 / AVAL_CADENCE_HZ)
        .clamp(AVAL_MIN_DURATION_SEC, AVAL_MAX_DURATION_SEC);
    TripTiming {
        duration_sec,
        strides,
    }
}

/// Foot-down hold discretisation of linear progress (`gaitLockedPlantT`).
///
/// Divides the walk into [strides] equal cycles. Within each cycle the foot is
/// planted (held) for the first [GAIT_HOLD_FRAC] (contact), then advances over
/// the remaining float phase. Prevents foot-sliding during ground contact.
pub fn gait_locked_plant_t(t_linear: f64, strides: u32) -> f64 {
    if strides == 0 {
        return t_linear.clamp(0.0, 1.0);
    }
    let s = strides as f64;
    let raw = (t_linear * s).clamp(0.0, s);
    let stride_idx = (raw.floor() as u32).min(strides - 1);
    let phase = raw - stride_idx as f64;
    let stride_t = if phase < GAIT_HOLD_FRAC {
        stride_idx as f64
    } else {
        stride_idx as f64 + (phase - GAIT_HOLD_FRAC) / (1.0 - GAIT_HOLD_FRAC)
    };
    (stride_t / s).clamp(0.0, 1.0)
}

/// Plant progress for one tick, honouring turn-before-plant hold.
///
/// When [require_visual] is true and [route_settled] is false, returns 0
/// regardless of [t_linear] (moonwalk guard).
pub fn plant_progress_t(
    t_linear: f64,
    strides: u32,
    require_visual: bool,
    route_settled: bool,
) -> f64 {
    if require_visual && !route_settled {
        return 0.0;
    }
    gait_locked_plant_t(t_linear.clamp(0.0, 1.0), strides)
}

/// Pure plant FSM: hold until route drained, then stride-locked advance.
///
/// Host drives settle via [`PlantController::set_route_settled`]; ticks advance
/// wall-clock only after settle when hard settle is required.
#[derive(Clone, Debug)]
pub struct PlantController {
    /// Hard settle (multi-hop facing change).
    pub require_visual: bool,
    /// Footfalls for gait quantisation.
    pub strides: u32,
    /// Planned wall-clock duration once plant is free to move.
    pub duration_sec: f64,
    /// Elapsed plant time (does not advance while held).
    pub elapsed_sec: f64,
    /// Host reports AVAL route drained + target visual active.
    pub route_settled: bool,
    /// Optional from/to for diagnostics.
    pub from_visual: Option<String>,
    /// Target walk state.
    pub to_state: String,
    /// Ring hop count at start.
    pub ring_hops: u32,
}

impl PlantController {
    /// Begin a plant trip after resolving walk state + floor distance.
    pub fn start(from_visual: Option<&str>, to_state: &str, dist: f64) -> Self {
        let hops = walk_ring_hops(from_visual, to_state);
        let require_visual = require_visual_settle(from_visual, to_state);
        let timing = aval_trip_timing(dist);
        // Soft path (idle→walk / same bank): plant may start immediately;
        // host still may delay for decode — we do not force hold.
        let route_settled = !require_visual;
        Self {
            require_visual,
            strides: timing.strides,
            duration_sec: timing.duration_sec,
            elapsed_sec: 0.0,
            route_settled,
            from_visual: from_visual.map(|s| s.to_string()),
            to_state: to_state.to_string(),
            ring_hops: hops,
        }
    }

    /// Host: multi-hop finished and body is on target walk loop.
    pub fn set_route_settled(&mut self, settled: bool) {
        self.route_settled = settled;
    }

    /// Advance plant by [dt] seconds. Time only accumulates when not held.
    pub fn tick(&mut self, dt: f64) -> PlantSample {
        let held = self.require_visual && !self.route_settled;
        if !held && dt > 0.0 {
            self.elapsed_sec += dt;
        }
        self.sample()
    }

    /// Current plant sample without advancing time.
    pub fn sample(&self) -> PlantSample {
        let held = self.require_visual && !self.route_settled;
        if held {
            return PlantSample {
                t_linear: 0.0,
                t: 0.0,
                held: true,
                finished: false,
            };
        }
        let dur = self.duration_sec.max(0.01);
        let t_linear = (self.elapsed_sec / dur).clamp(0.0, 1.0);
        let t = gait_locked_plant_t(t_linear, self.strides);
        PlantSample {
            t_linear,
            t,
            held: false,
            finished: t_linear >= 1.0,
        }
    }

    /// Suggested settle timeout matching Dart `_waitAvalSettled`.
    pub fn settle_timeout_ms(&self) -> u32 {
        if self.require_visual {
            hard_settle_timeout_ms(self.ring_hops)
        } else {
            SOFT_SETTLE_TIMEOUT_MS
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    #[test]
    fn ring_hops_same_and_adjacent() {
        assert_eq!(walk_ring_hops(Some("walk_east"), "walk_east"), 0);
        assert_eq!(walk_ring_hops(Some("east"), "east"), 0);
        assert_eq!(walk_ring_hops(Some("walk_east"), "walk_southeast"), 1);
        assert_eq!(walk_ring_hops(Some("walk_east"), "walk_northeast"), 1);
        // SE → NW: shortest is 4 hops either way.
        assert_eq!(
            walk_ring_hops(Some("walk_southeast"), "walk_northwest"),
            4
        );
        // N → S: 4 hops.
        assert_eq!(walk_ring_hops(Some("walk_north"), "walk_south"), 4);
        // E → W: 4 hops.
        assert_eq!(walk_ring_hops(Some("walk_east"), "walk_west"), 4);
        // E → S: 2 hops.
        assert_eq!(walk_ring_hops(Some("walk_east"), "walk_south"), 2);
        // bare vs walk_ prefix.
        assert_eq!(walk_ring_hops(Some("southeast"), "walk_northwest"), 4);
    }

    #[test]
    fn require_visual_on_multi_hop_not_from_idle() {
        assert!(require_visual_settle(
            Some("walk_southeast"),
            "walk_northwest"
        ));
        assert!(!require_visual_settle(Some("idle"), "walk_northwest"));
        assert!(!require_visual_settle(None, "walk_east"));
        assert!(!require_visual_settle(Some("intro"), "walk_south"));
        // Same facing: 0 hops → soft.
        assert!(!require_visual_settle(
            Some("walk_east"),
            "walk_east"
        ));
        // Adjacent 1 hop (E→SE): soft — hard-cut packs flip quickly.
        assert!(!require_visual_settle(
            Some("walk_east"),
            "walk_southeast"
        ));
        // 2+ hops (E→S): hard settle.
        assert!(require_visual_settle(
            Some("walk_east"),
            "walk_south"
        ));
    }

    #[test]
    fn trip_timing_stride_and_clamp() {
        // dist 0.05 → 1 stride, duration = 0.5 clamped up? 1/2 = 0.5, min 0.22.
        let t0 = aval_trip_timing(0.05);
        assert_eq!(t0.strides, 1);
        assert_abs_diff_eq!(t0.duration_sec, 0.5, epsilon = 1e-12);

        // dist 0.10 → 1 stride.
        let t1 = aval_trip_timing(0.10);
        assert_eq!(t1.strides, 1);
        assert_abs_diff_eq!(t1.duration_sec, 0.5, epsilon = 1e-12);

        // dist 0.11 → 2 strides, duration 1.0.
        let t2 = aval_trip_timing(0.11);
        assert_eq!(t2.strides, 2);
        assert_abs_diff_eq!(t2.duration_sec, 1.0, epsilon = 1e-12);

        // dist 0.50 → 5 strides, 2.5s.
        let t5 = aval_trip_timing(0.50);
        assert_eq!(t5.strides, 5);
        assert_abs_diff_eq!(t5.duration_sec, 2.5, epsilon = 1e-12);

        // Long path: 20 strides → 10s, under 12s cap (no forced skate).
        let t_long = aval_trip_timing(2.0); // 20 strides → 10s.
        assert_eq!(t_long.strides, 20);
        assert_abs_diff_eq!(t_long.duration_sec, 10.0, epsilon = 1e-12);
        // Extreme: clamp duration at 12.0; strides still from dist.
        let t_extreme = aval_trip_timing(4.0); // 40 strides → 20s → clamp 12.
        assert_eq!(t_extreme.strides, 40);
        assert_abs_diff_eq!(t_extreme.duration_sec, 12.0, epsilon = 1e-12);

        // Tiny dist still ≥ 1 stride; duration max(0.22, 0.5).
        let t_tiny = aval_trip_timing(0.0);
        assert_eq!(t_tiny.strides, 1);
        assert!(t_tiny.duration_sec >= AVAL_MIN_DURATION_SEC);
    }

    #[test]
    fn gait_locked_holds_then_advances() {
        let strides = 4;
        // Mid-hold of first stride: still at 0.
        let hold = gait_locked_plant_t(0.05 / strides as f64, strides);
        // t_linear = 0.05/4 = 0.0125 → raw = 0.05, phase 0.05 < 0.35 → strideT=0
        assert_abs_diff_eq!(hold, 0.0, epsilon = 1e-12);

        // Start of float phase on first stride (hold frac boundary).
        let mid_stride = GAIT_HOLD_FRAC / strides as f64;
        let at_hold_end = gait_locked_plant_t(mid_stride, strides);
        assert_abs_diff_eq!(at_hold_end, 0.0, epsilon = 1e-12);

        // End of first cycle (raw ≈ 1) → t ≈ 1/4.
        let end_first = gait_locked_plant_t(1.0 / strides as f64, strides);
        assert_abs_diff_eq!(end_first, 0.25, epsilon = 1e-9);

        assert_abs_diff_eq!(gait_locked_plant_t(0.0, strides), 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(gait_locked_plant_t(1.0, strides), 1.0, epsilon = 1e-9);
        // strides 0: pass-through clamp.
        assert_abs_diff_eq!(gait_locked_plant_t(0.3, 0), 0.3, epsilon = 1e-12);
    }

    /// SE→NW moonwalk regression: plant stays 0 until route_settled.
    #[test]
    fn moonwalk_regression_hold_until_route_settled() {
        let mut plant =
            PlantController::start(Some("walk_southeast"), "walk_northwest", 0.30);
        assert_eq!(plant.ring_hops, 4);
        assert!(plant.require_visual);
        assert!(!plant.route_settled);

        // Simulate multi-hop turn frames while body still SE / mid-route.
        for _ in 0..20 {
            let s = plant.tick(1.0 / 30.0);
            assert!(s.held, "must hold during ring hops");
            assert_abs_diff_eq!(s.t, 0.0, epsilon = 1e-12);
            assert_abs_diff_eq!(s.t_linear, 0.0, epsilon = 1e-12);
            assert!(!s.finished);
        }
        // Elapsed must not have advanced while held.
        assert_abs_diff_eq!(plant.elapsed_sec, 0.0, epsilon = 1e-12);

        // Host: route drained, visual == walk_northwest.
        plant.set_route_settled(true);
        let after = plant.tick(0.0);
        assert!(!after.held);
        assert_abs_diff_eq!(after.t, 0.0, epsilon = 1e-12);

        // Now plant may advance; half duration → progress > 0.
        let half = plant.duration_sec * 0.5;
        let mid = plant.tick(half);
        assert!(!mid.held);
        assert!(mid.t > 0.0, "plant must advance after settle, t={}", mid.t);
        assert!(mid.t_linear > 0.4 && mid.t_linear < 0.6);

        // Finish trip.
        let end = plant.tick(plant.duration_sec);
        assert!(end.finished);
        assert_abs_diff_eq!(end.t_linear, 1.0, epsilon = 1e-9);
        assert_abs_diff_eq!(end.t, 1.0, epsilon = 1e-9);
    }

    #[test]
    fn idle_to_walk_does_not_hard_hold() {
        let mut plant = PlantController::start(Some("idle"), "walk_east", 0.20);
        assert!(!plant.require_visual);
        assert!(plant.route_settled);
        let s = plant.tick(0.05);
        assert!(!s.held);
        assert!(s.t_linear > 0.0);
    }

    #[test]
    fn plant_progress_helper_mirrors_controller() {
        assert_abs_diff_eq!(
            plant_progress_t(0.5, 4, true, false),
            0.0,
            epsilon = 1e-12
        );
        let unlocked = plant_progress_t(0.5, 4, true, true);
        assert!(unlocked > 0.0);
        assert_abs_diff_eq!(
            unlocked,
            gait_locked_plant_t(0.5, 4),
            epsilon = 1e-12
        );
    }

    #[test]
    fn settle_timeout_scales_with_hops() {
        let se_nw =
            PlantController::start(Some("walk_southeast"), "walk_northwest", 0.2);
        assert_eq!(se_nw.settle_timeout_ms(), 4000 + 4 * 1200);
        let idle = PlantController::start(Some("idle"), "walk_east", 0.2);
        assert_eq!(idle.settle_timeout_ms(), SOFT_SETTLE_TIMEOUT_MS);
    }
}
