//! Retarget ARDY walk onto a start→end floor path.
//!
//! Mirrors Dart `ardy_path_retarget.dart`. Stability rules (anti-schizo /
//! anti-moonwalk):
//! 1. Use a **stable heading window** of the bake — circle walks turn ~130°.
//! 2. Advance that gait **sequentially** (1 cycle frame per output frame).
//! 3. Facing (mirror_x) chosen **once** for a straight trip.
//! 4. Plant lerps with ease-in-out at constant arc speed.

use crate::ardy_motion::{ArdyFrame, ArdyMotion};
use crate::scene_point::ScenePoint;

fn hypot(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}

fn angle_diff(a: f64, b: f64) -> f64 {
    let mut d = a - b;
    while d > std::f64::consts::PI {
        d -= 2.0 * std::f64::consts::PI;
    }
    while d < -std::f64::consts::PI {
        d += 2.0 * std::f64::consts::PI;
    }
    d
}

/// Circular mean of angles (Dart `_circularMean`).
fn circular_mean(angles: &[f64]) -> f64 {
    if angles.is_empty() {
        return 0.0;
    }
    let mut sx = 0.0;
    let mut sy = 0.0;
    for &a in angles {
        sx += a.cos();
        sy += a.sin();
    }
    let n = angles.len() as f64;
    (sy / n).atan2(sx / n)
}

/// Circular variance in [0, 1] (0 = all same heading).
fn circular_variance(angles: &[f64]) -> f64 {
    if angles.is_empty() {
        return 1.0;
    }
    let mut sx = 0.0;
    let mut sy = 0.0;
    for &a in angles {
        sx += a.cos();
        sy += a.sin();
    }
    let r = hypot(sx, sy) / angles.len() as f64;
    1.0 - r
}

/// Contiguous gait window pick result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StableGaitWindow {
    /// Start frame index in cycle.
    pub start: usize,
    /// Window length in frames.
    pub length: usize,
    /// Circular mean heading of the window.
    pub mean_heading: f64,
}

/// Preferred "forward" heading of a cycle (radians, atan2(dy,dx) screen space).
pub fn cycle_preferred_heading(cycle: &ArdyMotion) -> f64 {
    let hs: Vec<f64> = cycle
        .frames
        .iter()
        .filter_map(|f| f.heading)
        .collect();
    if hs.is_empty() {
        return 0.0;
    }
    circular_mean(&hs)
}

/// Whether the sprite should be mirrored so baked facing matches `walk_heading`.
///
/// True when the shortest turn from `cycle_facing` to `walk_heading` is more
/// than 90° (prefer flip over reverse play).
pub fn should_mirror_for_walk(walk_heading: f64, cycle_facing: f64) -> bool {
    angle_diff(walk_heading, cycle_facing).abs() > (std::f64::consts::PI * 0.5)
}

/// Contiguous gait slice with the most stable body heading.
pub fn pick_stable_gait_window(
    cycle: &ArdyMotion,
    prefer_len: usize,
    min_len: usize,
    max_len: usize,
) -> StableGaitWindow {
    let n = cycle.frame_count();
    if n == 0 {
        return StableGaitWindow {
            start: 0,
            length: 0,
            mean_heading: 0.0,
        };
    }
    let headings: Vec<Option<f64>> = cycle.frames.iter().map(|f| f.heading).collect();
    let known = headings.iter().filter(|h| h.is_some()).count();
    if known < min_len {
        return StableGaitWindow {
            start: 0,
            length: n,
            mean_heading: cycle_preferred_heading(cycle),
        };
    }

    let mut best_start = 0usize;
    let mut best_len = prefer_len.min(n);
    let mut best_var = f64::INFINITY;
    let mut best_mean = 0.0;

    let lo = min_len.min(n);
    let hi = max_len.min(n);
    for len in lo..=hi {
        let max_start = n - len;
        for start in 0..=max_start {
            let slice: Vec<f64> = (0..len)
                .filter_map(|i| headings[start + i])
                .collect();
            if slice.len() < lo {
                continue;
            }
            let v = circular_variance(&slice);
            // Slight preference for prefer_len.
            let score = v + (len as i32 - prefer_len as i32).unsigned_abs() as f64 * 0.0005;
            if score < best_var {
                best_var = score;
                best_start = start;
                best_len = len;
                best_mean = circular_mean(&slice);
            }
        }
    }

    StableGaitWindow {
        start: best_start,
        length: best_len,
        mean_heading: best_mean,
    }
}

/// Default window params matching Dart (`preferLen=12, minLen=8, maxLen=16`).
pub fn pick_stable_gait_window_default(cycle: &ArdyMotion) -> StableGaitWindow {
    pick_stable_gait_window(cycle, 12, 8, 16)
}

/// Frames of `cycle` in the stable gait window (contiguous, no wrap).
pub fn stable_gait_frames(cycle: &ArdyMotion) -> Vec<ArdyFrame> {
    let w = pick_stable_gait_window_default(cycle);
    if w.length == 0 || cycle.frames.is_empty() {
        return cycle.frames.clone();
    }
    cycle.frames[w.start..w.start + w.length].to_vec()
}

/// Result of a one-shot walk retarget.
#[derive(Clone, Debug)]
pub struct RetargetWalk {
    /// Retargeted motion.
    pub motion: ArdyMotion,
    /// Single mirror decision for the whole trip.
    pub mirror_x: bool,
}

/// Result of a polyline walk retarget.
#[derive(Clone, Debug)]
pub struct RetargetWalkAlong {
    /// Retargeted motion.
    pub motion: ArdyMotion,
    /// Per-frame mirror flags (same length as frames).
    pub mirror_per_frame: Vec<bool>,
}

/// Build a one-shot walk from `from` → `to`.
pub fn retarget_ardy_walk_to(
    cycle: &ArdyMotion,
    from: ScenePoint,
    to: ScenePoint,
    speed: f64,
    fps: Option<f64>,
) -> RetargetWalk {
    if cycle.frames.is_empty() {
        return RetargetWalk {
            motion: ArdyMotion {
                fps: fps.unwrap_or(cycle.fps),
                frames: vec![ArdyFrame::new(to.x, to.y, to.depth)],
                bones: cycle.bones.clone(),
                text: "walk to (empty cycle)".into(),
                skeleton: cycle.skeleton.clone(),
                render: cycle.render.clone(),
                sprite_dir: cycle.sprite_dir.clone(),
                sprite_pattern: cycle.sprite_pattern.clone(),
                sprite_count: cycle.sprite_count,
                sprite_w: cycle.sprite_w,
                sprite_h: cycle.sprite_h,
            },
            mirror_x: false,
        };
    }

    let use_fps = fps.unwrap_or(if cycle.fps > 0.0 { cycle.fps } else { 20.0 });
    let dist = hypot(to.x - from.x, to.y - from.y);

    // Very short hop: snap plant, hold idle-ish first gait frame.
    if dist < 0.008 {
        let gait = stable_gait_frames(cycle);
        let src = if !gait.is_empty() {
            &gait[0]
        } else {
            &cycle.frames[0]
        };
        let joints2d = src.joints2d.as_ref().map(|js| {
            js.iter()
                .map(|j| [j[0] - src.x + to.x, j[1] - src.y + to.y])
                .collect()
        });
        return RetargetWalk {
            motion: ArdyMotion {
                fps: use_fps,
                frames: vec![ArdyFrame {
                    x: to.x,
                    y: to.y,
                    depth: clamp01(1.0 - to.y),
                    joints2d,
                    sprite_index: Some(src.sprite_index.unwrap_or(0)),
                    heading: src.heading,
                    sprite_anchor: src.sprite_anchor,
                }],
                bones: cycle.bones.clone(),
                text: "walk to (hop)".into(),
                skeleton: cycle.skeleton.clone(),
                render: cycle.render.clone(),
                sprite_dir: cycle.sprite_dir.clone(),
                sprite_pattern: cycle.sprite_pattern.clone(),
                sprite_count: cycle.sprite_count,
                sprite_w: cycle.sprite_w,
                sprite_h: cycle.sprite_h,
            },
            mirror_x: false,
        };
    }

    let walk_heading = (to.y - from.y).atan2(to.x - from.x);
    let window = pick_stable_gait_window_default(cycle);
    let gait = stable_gait_frames(cycle);
    let cycle_facing = if window.length > 0 {
        window.mean_heading
    } else {
        cycle_preferred_heading(cycle)
    };
    let mirror = should_mirror_for_walk(walk_heading, cycle_facing);

    let gait_n = if !gait.is_empty() {
        gait.len()
    } else {
        cycle.frame_count()
    };
    let n_frames = gait_aligned_frame_count(dist, speed, use_fps, gait_n, 10);
    let mut frames = Vec::with_capacity(n_frames);
    for i in 0..n_frames {
        let raw_t = if n_frames == 1 {
            1.0
        } else {
            i as f64 / (n_frames - 1) as f64
        };
        let t = ease_in_out_plant(raw_t);
        let x = from.x + (to.x - from.x) * t;
        let y = from.y + (to.y - from.y) * t;
        let depth = clamp01(1.0 - y);

        let phase = i % gait_n;
        let src = if !gait.is_empty() {
            &gait[phase]
        } else {
            &cycle.frames[phase]
        };

        let joints2d = src.joints2d.as_ref().and_then(|js| {
            if js.is_empty() {
                return None;
            }
            let ox = src.x;
            let oy = src.y;
            Some(
                js.iter()
                    .map(|j| [j[0] - ox + x, j[1] - oy + y])
                    .collect(),
            )
        });

        frames.push(ArdyFrame {
            x,
            y,
            depth,
            joints2d,
            sprite_index: Some(
                src.sprite_index
                    .unwrap_or((window.start + phase) as i32),
            ),
            heading: Some(walk_heading),
            sprite_anchor: src.sprite_anchor,
        });
    }

    RetargetWalk {
        motion: ArdyMotion {
            fps: use_fps,
            frames,
            bones: cycle.bones.clone(),
            text: format!("walk to ({:.3}, {:.3})", to.x, to.y),
            skeleton: cycle.skeleton.clone(),
            render: cycle.render.clone(),
            sprite_dir: cycle.sprite_dir.clone(),
            sprite_pattern: cycle.sprite_pattern.clone(),
            sprite_count: cycle.sprite_count,
            sprite_w: cycle.sprite_w,
            sprite_h: cycle.sprite_h,
        },
        mirror_x: mirror,
    }
}

/// Retarget along a polyline of waypoints (guide path).
pub fn retarget_ardy_walk_along(
    cycle: &ArdyMotion,
    waypoints: &[ScenePoint],
    speed: f64,
    fps: Option<f64>,
) -> RetargetWalkAlong {
    let mut pts: Vec<ScenePoint> = Vec::new();
    for (i, &wp) in waypoints.iter().enumerate() {
        if i == 0
            || hypot(wp.x - waypoints[i - 1].x, wp.y - waypoints[i - 1].y) > 0.008
        {
            pts.push(wp);
        }
    }
    if pts.len() < 2 {
        let p = pts
            .last()
            .copied()
            .unwrap_or(ScenePoint::new(0.5, 0.8, 0.2));
        let hop = retarget_ardy_walk_to(cycle, p, p, speed, fps);
        let mirror_per_frame = hop.motion.frames.iter().map(|_| hop.mirror_x).collect();
        return RetargetWalkAlong {
            motion: hop.motion,
            mirror_per_frame,
        };
    }

    let use_fps = fps.unwrap_or(if cycle.fps > 0.0 { cycle.fps } else { 20.0 });
    let gait = stable_gait_frames(cycle);
    let gait_n = if !gait.is_empty() {
        gait.len()
    } else {
        cycle.frame_count()
    };
    let window = pick_stable_gait_window_default(cycle);
    let cycle_facing = if window.length > 0 {
        window.mean_heading
    } else {
        cycle_preferred_heading(cycle)
    };

    // Build pathPts as ends of non-tiny segments.
    let mut path_pts = vec![pts[0]];
    for s in 0..pts.len() - 1 {
        let d = hypot(pts[s + 1].x - pts[s].x, pts[s + 1].y - pts[s].y);
        if d >= 0.008 {
            path_pts.push(pts[s + 1]);
        }
    }

    let mut seg_lens: Vec<f64> = Vec::new();
    let mut total_len = 0.0;
    for s in 0..path_pts.len().saturating_sub(1) {
        let d = hypot(
            path_pts[s + 1].x - path_pts[s].x,
            path_pts[s + 1].y - path_pts[s].y,
        );
        seg_lens.push(d);
        total_len += d;
    }

    if total_len < 0.008 || seg_lens.is_empty() {
        let last = *pts.last().unwrap();
        let hop = retarget_ardy_walk_to(cycle, pts[0], last, speed, Some(use_fps));
        let mirror_per_frame = hop.motion.frames.iter().map(|_| hop.mirror_x).collect();
        return RetargetWalkAlong {
            motion: hop.motion,
            mirror_per_frame,
        };
    }

    let total_frames = gait_aligned_frame_count(total_len, speed, use_fps, gait_n, 12);
    let mut all_frames = Vec::with_capacity(total_frames);
    let mut mirrors = Vec::with_capacity(total_frames);
    let mut gait_phase = 0usize;
    let mut prev_mirror: Option<bool> = None;

    for fi in 0..total_frames {
        let raw_t = if total_frames == 1 {
            1.0
        } else {
            fi as f64 / (total_frames - 1) as f64
        };
        let eased = ease_in_out_plant(raw_t);
        let along = eased * total_len;

        let mut remain = along;
        let mut s_idx = 0usize;
        while s_idx < seg_lens.len() - 1 && remain > seg_lens[s_idx] {
            remain -= seg_lens[s_idx];
            s_idx += 1;
        }
        let seg_len = seg_lens[s_idx].max(1e-9);
        let local_t = (remain / seg_len).clamp(0.0, 1.0);
        let from = path_pts[s_idx];
        let to = path_pts[s_idx + 1];
        let x = from.x + (to.x - from.x) * local_t;
        let y = from.y + (to.y - from.y) * local_t;
        let depth = clamp01(1.0 - y);
        let walk_heading = (to.y - from.y).atan2(to.x - from.x);

        let mut mirror = should_mirror_for_walk(walk_heading, cycle_facing);
        // Hysteresis: only flip facing when the segment turn is meaningful.
        if let Some(pm) = prev_mirror {
            if mirror != pm {
                let turn = if s_idx > 0 {
                    segment_turn_abs(path_pts[s_idx - 1], path_pts[s_idx], path_pts[s_idx + 1])
                } else {
                    std::f64::consts::PI
                };
                if turn < (std::f64::consts::PI * 0.25) {
                    // < 45° — keep previous facing.
                    mirror = pm;
                }
            }
        }
        prev_mirror = Some(mirror);

        let phase = gait_phase % gait_n;
        gait_phase += 1;
        let src = if !gait.is_empty() {
            &gait[phase]
        } else {
            &cycle.frames[phase]
        };

        let joints2d = src.joints2d.as_ref().and_then(|js| {
            if js.is_empty() {
                return None;
            }
            let ox = src.x;
            let oy = src.y;
            Some(
                js.iter()
                    .map(|j| [j[0] - ox + x, j[1] - oy + y])
                    .collect(),
            )
        });

        all_frames.push(ArdyFrame {
            x,
            y,
            depth,
            joints2d,
            sprite_index: Some(
                src.sprite_index
                    .unwrap_or((window.start + phase) as i32),
            ),
            heading: Some(walk_heading),
            sprite_anchor: src.sprite_anchor,
        });
        mirrors.push(mirror);
    }

    if all_frames.is_empty() {
        let last = *pts.last().unwrap();
        let hop = retarget_ardy_walk_to(cycle, last, last, speed, Some(use_fps));
        let mirror_per_frame = hop.motion.frames.iter().map(|_| hop.mirror_x).collect();
        return RetargetWalkAlong {
            motion: hop.motion,
            mirror_per_frame,
        };
    }

    let last = path_pts.last().unwrap();
    RetargetWalkAlong {
        motion: ArdyMotion {
            fps: use_fps,
            frames: all_frames,
            bones: cycle.bones.clone(),
            text: format!(
                "walk path ({} pts → {:.2},{:.2})",
                path_pts.len(),
                last.x,
                last.y
            ),
            skeleton: cycle.skeleton.clone(),
            render: cycle.render.clone(),
            sprite_dir: cycle.sprite_dir.clone(),
            sprite_pattern: cycle.sprite_pattern.clone(),
            sprite_count: cycle.sprite_count,
            sprite_w: cycle.sprite_w,
            sprite_h: cycle.sprite_h,
        },
        mirror_per_frame: mirrors,
    }
}

/// Soft start/stop for plant only (smoothstep). Gait phase stays linear.
fn ease_in_out_plant(t: f64) -> f64 {
    let x = t.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Frame count at natural fps, snapped up to a whole number of gait cycles.
fn gait_aligned_frame_count(
    dist: f64,
    speed: f64,
    fps: f64,
    gait_n: usize,
    min_frames: usize,
) -> usize {
    let duration_sec = (dist / speed.max(1e-3)).max(0.35);
    let mut n = ((duration_sec * fps).round() as usize).max(min_frames);
    if gait_n > 1 {
        let cycles = (n as f64 / gait_n as f64).ceil().max(1.0) as usize;
        n = cycles * gait_n;
    }
    n
}

fn segment_turn_abs(a: ScenePoint, b: ScenePoint, c: ScenePoint) -> f64 {
    let h0 = (b.y - a.y).atan2(b.x - a.x);
    let h1 = (c.y - b.y).atan2(c.x - b.x);
    angle_diff(h1, h0).abs()
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use std::f64::consts::PI;
    use std::sync::OnceLock;

    fn load_demo_cycle() -> &'static ArdyMotion {
        static CYCLE: OnceLock<ArdyMotion> = OnceLock::new();
        CYCLE.get_or_init(|| {
            // Prefer crate fixture; fall back to Flutter asset path.
            let candidates = [
                "fixtures/ardy_walk_demo.json",
                "scene-engine/fixtures/ardy_walk_demo.json",
                "../crm-flutter/assets/scene/ardy_walk_demo.json",
                "crm-flutter/assets/scene/ardy_walk_demo.json",
            ];
            for path in candidates {
                if let Ok(raw) = std::fs::read_to_string(path) {
                    return ArdyMotion::parse(&raw).expect("parse ardy walk demo");
                }
            }
            // Synthetic circle-ish cycle if fixture missing (CI safety).
            synthetic_circle_cycle()
        })
    }

    fn synthetic_circle_cycle() -> ArdyMotion {
        let mut frames = Vec::new();
        for i in 0..40 {
            // Headings sweep ~0.5 → ~2.8 like the real bake.
            let heading = 0.5 + (i as f64 / 39.0) * 2.3;
            // Stable mid-window: frames 8..20 with nearly constant heading ~1.85
            let heading = if (8..20).contains(&i) {
                1.85 + (i as f64 - 14.0) * 0.01
            } else {
                heading
            };
            let span = 0.04 + 0.02 * ((i as f64) * 0.4).sin().abs();
            frames.push(ArdyFrame {
                x: 0.5,
                y: 0.78,
                depth: 0.22,
                joints2d: Some(vec![
                    [0.5 - span / 2.0, 0.9],
                    [0.5 + span / 2.0, 0.9],
                    [0.5, 0.5],
                ]),
                sprite_index: Some(i as i32),
                heading: Some(heading),
                sprite_anchor: Some([0.5, 0.95]),
            });
        }
        ArdyMotion {
            fps: 20.0,
            frames,
            bones: vec![(0, 1), (1, 2)],
            text: "synthetic circle".into(),
            skeleton: "core27".into(),
            render: "skinned_sprite".into(),
            sprite_dir: Some("assets/scene/ardy_walk_sprites".into()),
            sprite_pattern: "frame_{i:03d}.png".into(),
            sprite_count: Some(40),
            sprite_w: 160,
            sprite_h: 240,
        }
    }

    #[test]
    fn retarget_walk_ends_at_target_feet() {
        let cycle = load_demo_cycle();
        let from = ScenePoint::new(0.5, 0.8, 0.2);
        let to = ScenePoint::new(0.217, 0.693, 0.307);
        let walk = retarget_ardy_walk_to(cycle, from, to, 0.28, None);
        assert!(walk.motion.frame_count() > 2);
        let last = walk.motion.frames.last().unwrap();
        assert_abs_diff_eq!(last.x, to.x, epsilon = 1e-6);
        assert_abs_diff_eq!(last.y, to.y, epsilon = 1e-6);
        let joints = last.joints2d.as_ref().expect("joints rebased");
        // Real fixture: 27; synthetic fallback: ≥2 feet joints.
        assert!(joints.len() >= 2);
        let first = &walk.motion.frames[0];
        assert_abs_diff_eq!(first.x, from.x, epsilon = 1e-6);
        assert_abs_diff_eq!(first.y, from.y, epsilon = 1e-6);
    }

    #[test]
    fn short_hop_still_has_frames() {
        let cycle = load_demo_cycle();
        let walk = retarget_ardy_walk_to(
            cycle,
            ScenePoint::new(0.5, 0.7, 0.3),
            ScenePoint::new(0.51, 0.7, 0.3),
            0.28,
            None,
        );
        // dist = 0.01 > 0.008 so not hop-snap; still has frames
        assert!(walk.motion.frame_count() >= 1);
        // true hop
        let hop = retarget_ardy_walk_to(
            cycle,
            ScenePoint::new(0.5, 0.7, 0.3),
            ScenePoint::new(0.505, 0.7, 0.3),
            0.28,
            None,
        );
        assert_eq!(hop.motion.frame_count(), 1);
    }

    #[test]
    fn never_moonwalks_plant_advances_toward_target() {
        let cycle = load_demo_cycle();
        let from = ScenePoint::new(0.2, 0.8, 0.2);
        let to = ScenePoint::new(0.8, 0.8, 0.2);
        let walk = retarget_ardy_walk_to(cycle, from, to, 0.15, None);
        let frames = &walk.motion.frames;
        for i in 1..frames.len() {
            assert!(frames[i].x >= frames[i - 1].x - 1e-9);
        }
    }

    #[test]
    fn gait_advances_sequentially() {
        let cycle = load_demo_cycle();
        let walk = retarget_ardy_walk_to(
            cycle,
            ScenePoint::new(0.2, 0.75, 0.25),
            ScenePoint::new(0.8, 0.75, 0.25),
            0.15,
            None,
        );
        let gait = stable_gait_frames(cycle);
        assert!(gait.len() >= 8);
        let sis: Vec<i32> = walk
            .motion
            .frames
            .iter()
            .map(|f| f.sprite_index.unwrap_or(0))
            .collect();
        let gait_si: Vec<i32> = gait.iter().map(|f| f.sprite_index.unwrap_or(0)).collect();
        for i in 1..sis.len() {
            let prev = sis[i - 1];
            let cur = sis[i];
            let prev_idx = gait_si.iter().position(|&s| s == prev);
            let cur_idx = gait_si.iter().position(|&s| s == cur);
            let (Some(prev_idx), Some(cur_idx)) = (prev_idx, cur_idx) else {
                continue;
            };
            let step = (cur_idx + gait.len() - prev_idx) % gait.len();
            let ok = step == 1
                || step == 0
                || (prev_idx == gait.len() - 1 && cur_idx == 0);
            assert!(
                ok,
                "frame {i}: gait {prev_idx} → {cur_idx} (step {step})"
            );
        }
    }

    #[test]
    fn stable_gait_window_has_low_heading_variance() {
        let cycle = load_demo_cycle();
        let w = pick_stable_gait_window_default(cycle);
        assert!(w.length >= 8);
        assert!(w.length <= 16);
        let mut max_dev = 0.0;
        for i in 0..w.length {
            if let Some(h) = cycle.frames[w.start + i].heading {
                let d = angle_diff(h, w.mean_heading).abs();
                if d > max_dev {
                    max_dev = d;
                }
            }
        }
        assert!(max_dev < 20.0 * PI / 180.0);
    }

    #[test]
    fn mirror_is_stable_for_whole_trip() {
        let cycle = load_demo_cycle();
        let left = retarget_ardy_walk_to(
            cycle,
            ScenePoint::new(0.7, 0.75, 0.25),
            ScenePoint::new(0.2, 0.75, 0.25),
            0.2,
            None,
        );
        let right = retarget_ardy_walk_to(
            cycle,
            ScenePoint::new(0.2, 0.75, 0.25),
            ScenePoint::new(0.7, 0.75, 0.25),
            0.2,
            None,
        );
        for f in &left.motion.frames {
            let h = f.heading.expect("heading");
            assert_abs_diff_eq!(h, PI, epsilon = 0.05);
        }
        for f in &right.motion.frames {
            let h = f.heading.expect("heading");
            assert_abs_diff_eq!(h.abs(), 0.0, epsilon = 0.05);
        }
        let cycle_facing = pick_stable_gait_window_default(cycle).mean_heading;
        assert_eq!(
            left.mirror_x,
            should_mirror_for_walk(PI, cycle_facing)
        );
        assert_eq!(
            right.mirror_x,
            should_mirror_for_walk(0.0, cycle_facing)
        );
    }

    #[test]
    fn polyline_retarget_visits_waypoints() {
        let cycle = load_demo_cycle();
        let path = [
            ScenePoint::new(0.2, 0.85, 0.15),
            ScenePoint::new(0.4, 0.80, 0.20),
            ScenePoint::new(0.7, 0.78, 0.22),
        ];
        let walk = retarget_ardy_walk_along(cycle, &path, 0.2, None);
        assert!(walk.motion.frame_count() > 10);
        assert_eq!(walk.mirror_per_frame.len(), walk.motion.frame_count());
        assert_abs_diff_eq!(walk.motion.frames[0].x, path[0].x, epsilon = 0.05);
        let last = walk.motion.frames.last().unwrap();
        assert_abs_diff_eq!(last.x, path[2].x, epsilon = 0.02);
        assert_abs_diff_eq!(last.y, path[2].y, epsilon = 0.02);
        for i in 1..walk.motion.frames.len() {
            assert!(
                walk.motion.frames[i].x >= walk.motion.frames[i - 1].x - 0.02
            );
        }
    }

    #[test]
    fn shallow_bend_keeps_mirror_hysteresis() {
        let cycle = load_demo_cycle();
        let path = [
            ScenePoint::new(0.2, 0.80, 0.2),
            ScenePoint::new(0.45, 0.805, 0.2),
            ScenePoint::new(0.7, 0.80, 0.2),
        ];
        let walk = retarget_ardy_walk_along(cycle, &path, 0.2, None);
        let set: std::collections::HashSet<bool> =
            walk.mirror_per_frame.iter().copied().collect();
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn gait_aligned_frame_count_is_multiple_of_gait_window() {
        let cycle = load_demo_cycle();
        let walk = retarget_ardy_walk_to(
            cycle,
            ScenePoint::new(0.2, 0.8, 0.2),
            ScenePoint::new(0.7, 0.8, 0.2),
            0.15,
            None,
        );
        let gait_n = stable_gait_frames(cycle).len();
        assert!(gait_n > 1);
        assert_eq!(walk.motion.frame_count() % gait_n, 0);
    }

    #[test]
    fn ease_and_gait_helpers() {
        assert_abs_diff_eq!(ease_in_out_plant(0.0), 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(ease_in_out_plant(1.0), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(ease_in_out_plant(0.5), 0.5, epsilon = 1e-12);
        let n = gait_aligned_frame_count(0.5, 0.15, 20.0, 12, 10);
        assert_eq!(n % 12, 0);
        assert!(n >= 12);
    }
}
