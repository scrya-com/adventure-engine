//! ARDY motion payload — mirrors Dart `ardy_motion.dart`.
//!
//! Path mode: [`ArdyFrame::x`]/[`y`]/[`depth`] drive feet on the stage.
//! Skeleton mode: stick [`ArdyFrame::joints2d`] (debug / fallback).
//! Skinned sprite mode (`render == "skinned_sprite"`): LBS mesh bake.

use crate::scene_point::ScenePoint;
use serde::{Deserialize, Serialize};

/// One ARDY animation frame (feet plant + optional joints / sprite).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArdyFrame {
    /// Feet x (normalized background).
    pub x: f64,
    /// Feet y (normalized background, y-down).
    pub y: f64,
    /// Depth 0 near .. 1 far.
    pub depth: f64,
    /// Normalized background coords per joint, or `None` when path-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joints2d: Option<Vec<[f64; 2]>>,
    /// Index into sprite sequence (skinned bake).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "sprite_index")]
    pub sprite_index: Option<i32>,
    /// Root travel heading in floor plane (radians, atan2(dy, dx)).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<f64>,
    /// Sprite plant point in UV of the PNG (0..1), default bottom-center.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "sprite_anchor")]
    pub sprite_anchor: Option<[f64; 2]>,
}

impl ArdyFrame {
    /// Construct a path-only frame.
    pub fn new(x: f64, y: f64, depth: f64) -> Self {
        Self {
            x,
            y,
            depth,
            joints2d: None,
            sprite_index: None,
            heading: None,
            sprite_anchor: None,
        }
    }

    /// Feet plant as [`ScenePoint`].
    pub fn feet(&self) -> ScenePoint {
        ScenePoint::new(self.x, self.y, self.depth)
    }

    /// Parse from JSON object (Dart `ArdyFrame.fromJson`).
    pub fn from_json_value(json: &serde_json::Value) -> Option<Self> {
        let x = json.get("x")?.as_f64()?;
        let y = json.get("y")?.as_f64()?;
        let depth = json
            .get("depth")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| clamp01(1.0 - y));

        let joints2d = json.get("joints2d").and_then(|raw| {
            let arr = raw.as_array()?;
            let mut out = Vec::new();
            for j in arr {
                let pair = j.as_array()?;
                if pair.len() < 2 {
                    continue;
                }
                let jx = pair[0].as_f64()?;
                let jy = pair[1].as_f64()?;
                out.push([jx, jy]);
            }
            Some(out)
        });

        let sprite_anchor = json.get("sprite_anchor").and_then(|sa| {
            let arr = sa.as_array()?;
            if arr.len() < 2 {
                return None;
            }
            Some([arr[0].as_f64()?, arr[1].as_f64()?])
        });

        let sprite_index = json
            .get("sprite_index")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .map(|i| i as i32);

        let heading = json.get("heading").and_then(|v| v.as_f64());

        Some(Self {
            x,
            y,
            depth,
            joints2d,
            sprite_index,
            heading,
            sprite_anchor,
        })
    }
}

/// Full ARDY clip (walk cycle, idle, or retargeted path).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArdyMotion {
    /// Frames per second.
    pub fps: f64,
    /// Animation frames.
    pub frames: Vec<ArdyFrame>,
    /// Bone index pairs for stick debug draw.
    pub bones: Vec<(i32, i32)>,
    /// Human-readable label.
    #[serde(default)]
    pub text: String,
    /// Skeleton id (e.g. `core27`).
    #[serde(default = "default_skeleton")]
    pub skeleton: String,
    /// `sticks` | `skinned_sprite`.
    #[serde(default = "default_render")]
    pub render: String,
    /// Optional sprite directory (asset path, engine-agnostic string).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "sprite_dir")]
    pub sprite_dir: Option<String>,
    /// Sprite filename pattern.
    #[serde(default = "default_sprite_pattern", rename = "sprite_pattern")]
    pub sprite_pattern: String,
    /// Sprite frame count when skinned.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "sprite_count")]
    pub sprite_count: Option<i32>,
    /// Sprite pixel width.
    #[serde(default = "default_sprite_w", rename = "sprite_w")]
    pub sprite_w: i32,
    /// Sprite pixel height.
    #[serde(default = "default_sprite_h", rename = "sprite_h")]
    pub sprite_h: i32,
}

fn default_skeleton() -> String {
    "core27".into()
}
fn default_render() -> String {
    "sticks".into()
}
fn default_sprite_pattern() -> String {
    "frame_{i:03d}.png".into()
}
fn default_sprite_w() -> i32 {
    160
}
fn default_sprite_h() -> i32 {
    240
}

impl ArdyMotion {
    /// Construct with required fields; other metadata uses Dart defaults.
    pub fn new(fps: f64, frames: Vec<ArdyFrame>, bones: Vec<(i32, i32)>) -> Self {
        Self {
            fps,
            frames,
            bones,
            text: String::new(),
            skeleton: default_skeleton(),
            render: default_render(),
            sprite_dir: None,
            sprite_pattern: default_sprite_pattern(),
            sprite_count: None,
            sprite_w: default_sprite_w(),
            sprite_h: default_sprite_h(),
        }
    }

    /// True when skinned sprites are available.
    pub fn has_sprites(&self) -> bool {
        self.render == "skinned_sprite"
            && self
                .sprite_dir
                .as_ref()
                .is_some_and(|d| !d.is_empty())
            && self.sprite_count.unwrap_or(0) > 0
    }

    /// Frame count.
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Frame duration in microseconds (Dart `Duration` parity).
    pub fn frame_duration_us(&self) -> i64 {
        let fps = if self.fps <= 0.0 { 20.0 } else { self.fps };
        (1e6 / fps).round() as i64
    }

    /// Asset path for sprite frame `i` (engine-agnostic path string).
    pub fn sprite_asset_path(&self, i: i32) -> String {
        let dir = self
            .sprite_dir
            .as_deref()
            .unwrap_or("assets/scene/ardy_walk_sprites");
        let file = if self.sprite_pattern.contains("{i:03d}") {
            format!("frame_{i:03}.png")
        } else {
            self.sprite_pattern
                .replace("{i:03d}", &format!("{i:03}"))
        };
        format!("{dir}/{file}")
    }

    /// Parse from JSON value (Dart `ArdyMotion.fromJson`).
    pub fn from_json_value(json: &serde_json::Value) -> Self {
        let frames = json
            .get("frames")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(ArdyFrame::from_json_value)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let bones = json
            .get("bones")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|b| {
                        let pair = b.as_array()?;
                        if pair.len() < 2 {
                            return None;
                        }
                        let a = pair[0].as_i64().or_else(|| pair[0].as_f64().map(|f| f as i64))?
                            as i32;
                        let b = pair[1].as_i64().or_else(|| pair[1].as_f64().map(|f| f as i64))?
                            as i32;
                        Some((a, b))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let sprite_count = json
            .get("sprite_count")
            .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            .or_else(|| {
                json.get("frame_count")
                    .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
            })
            .map(|i| i as i32);

        Self {
            fps: json
                .get("fps")
                .and_then(|v| v.as_f64())
                .unwrap_or(20.0),
            frames,
            bones,
            text: json
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            skeleton: json
                .get("skeleton")
                .and_then(|v| v.as_str())
                .unwrap_or("core27")
                .to_string(),
            render: json
                .get("render")
                .and_then(|v| v.as_str())
                .unwrap_or("sticks")
                .to_string(),
            sprite_dir: json
                .get("sprite_dir")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            sprite_pattern: json
                .get("sprite_pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("frame_{i:03d}.png")
                .to_string(),
            sprite_count,
            sprite_w: json
                .get("sprite_w")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .map(|i| i as i32)
                .unwrap_or(160),
            sprite_h: json
                .get("sprite_h")
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .map(|i| i as i32)
                .unwrap_or(240),
        }
    }

    /// Parse from JSON string (Dart `ArdyMotion.parse`).
    pub fn parse(source: &str) -> Result<Self, serde_json::Error> {
        let v: serde_json::Value = serde_json::from_str(source)?;
        Ok(Self::from_json_value(&v))
    }

    /// Same pose/sprites, every frame's feet on `plant` (in-place loop).
    ///
    /// Joints are rebased relative to the old feet so the skeleton stays over
    /// the plant instead of skating back to the cycle origin.
    pub fn with_plant(&self, plant: ScenePoint) -> Self {
        let d = plant.depth;
        let frames = self
            .frames
            .iter()
            .map(|f| {
                let joints2d = f.joints2d.as_ref().map(|js| {
                    js.iter()
                        .map(|j| [j[0] - f.x + plant.x, j[1] - f.y + plant.y])
                        .collect()
                });
                ArdyFrame {
                    x: plant.x,
                    y: plant.y,
                    depth: d,
                    joints2d,
                    sprite_index: f.sprite_index,
                    heading: f.heading,
                    sprite_anchor: f.sprite_anchor,
                }
            })
            .collect();
        Self {
            fps: self.fps,
            frames,
            bones: self.bones.clone(),
            text: self.text.clone(),
            skeleton: self.skeleton.clone(),
            render: self.render.clone(),
            sprite_dir: self.sprite_dir.clone(),
            sprite_pattern: self.sprite_pattern.clone(),
            sprite_count: self.sprite_count,
            sprite_w: self.sprite_w,
            sprite_h: self.sprite_h,
        }
    }

    /// Walk-cycle frame closest to double-support (smallest foot span).
    pub fn idle_stance_frame_index(&self) -> usize {
        if self.frames.is_empty() {
            return 0;
        }
        let mut best_i = 0usize;
        let mut best_span = f64::INFINITY;
        for (i, frame) in self.frames.iter().enumerate() {
            let Some(joints) = frame.joints2d.as_ref() else {
                continue;
            };
            if joints.len() < 2 {
                continue;
            }
            // Two lowest joints on screen (y down) ≈ feet/ankles.
            let mut i0 = 0usize;
            let mut i1 = 1usize;
            if joints[1][1] > joints[0][1] {
                i0 = 1;
                i1 = 0;
            }
            for k in 2..joints.len() {
                let y = joints[k][1];
                if y > joints[i0][1] {
                    i1 = i0;
                    i0 = k;
                } else if y > joints[i1][1] {
                    i1 = k;
                }
            }
            let dx = joints[i0][0] - joints[i1][0];
            let dy = joints[i0][1] - joints[i1][1];
            let span = dx * dx + dy * dy;
            if span < best_span {
                best_span = span;
                best_i = i;
            }
        }
        best_i
    }

    /// Single-frame standing pose at `plant` (double-support from this cycle).
    pub fn hold_idle_at(&self, plant: ScenePoint) -> Self {
        if self.frames.is_empty() {
            return Self {
                fps: self.fps,
                frames: vec![ArdyFrame::new(plant.x, plant.y, plant.depth)],
                bones: self.bones.clone(),
                text: "idle".into(),
                skeleton: self.skeleton.clone(),
                render: self.render.clone(),
                sprite_dir: self.sprite_dir.clone(),
                sprite_pattern: self.sprite_pattern.clone(),
                sprite_count: self.sprite_count,
                sprite_w: self.sprite_w,
                sprite_h: self.sprite_h,
            };
        }
        let stance = self.idle_stance_frame_index();
        let src = &self.frames[stance];
        let d = plant.depth;
        let joints2d = src.joints2d.as_ref().map(|js| {
            js.iter()
                .map(|j| [j[0] - src.x + plant.x, j[1] - src.y + plant.y])
                .collect()
        });
        Self {
            fps: self.fps,
            frames: vec![ArdyFrame {
                x: plant.x,
                y: plant.y,
                depth: d,
                joints2d,
                sprite_index: Some(src.sprite_index.unwrap_or(stance as i32)),
                heading: src.heading,
                sprite_anchor: src.sprite_anchor,
            }],
            bones: self.bones.clone(),
            text: "idle".into(),
            skeleton: self.skeleton.clone(),
            render: self.render.clone(),
            sprite_dir: self.sprite_dir.clone(),
            sprite_pattern: self.sprite_pattern.clone(),
            sprite_count: self.sprite_count,
            sprite_w: self.sprite_w,
            sprite_h: self.sprite_h,
        }
    }
}

/// How the stage should present an ARDY clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArdyPlayMode {
    /// Feet follow path; AVAL walk cycles play.
    Path,
    /// Skinned sprite if available, else sticks; AVAL hidden.
    Skeleton,
    /// Path + AVAL + ARDY overlay.
    Both,
}

/// Parse play mode from string (Dart `ardyPlayModeFromString`).
pub fn ardy_play_mode_from_string(value: Option<&str>) -> ArdyPlayMode {
    match value {
        Some("path") => ArdyPlayMode::Path,
        Some("skeleton") => ArdyPlayMode::Skeleton,
        Some("both") | _ => ArdyPlayMode::Both,
    }
}

fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;

    fn tiny_cycle() -> ArdyMotion {
        ArdyMotion {
            fps: 20.0,
            frames: vec![
                ArdyFrame {
                    x: 0.5,
                    y: 0.8,
                    depth: 0.2,
                    joints2d: Some(vec![[0.48, 0.9], [0.52, 0.91], [0.5, 0.5]]),
                    sprite_index: Some(0),
                    heading: Some(0.0),
                    sprite_anchor: None,
                },
                ArdyFrame {
                    x: 0.5,
                    y: 0.8,
                    depth: 0.2,
                    joints2d: Some(vec![[0.46, 0.9], [0.54, 0.9], [0.5, 0.5]]),
                    sprite_index: Some(1),
                    heading: Some(0.1),
                    sprite_anchor: None,
                },
            ],
            bones: vec![(0, 1)],
            text: "tiny".into(),
            skeleton: "core27".into(),
            render: "sticks".into(),
            sprite_dir: None,
            sprite_pattern: default_sprite_pattern(),
            sprite_count: None,
            sprite_w: 160,
            sprite_h: 240,
        }
    }

    #[test]
    fn with_plant_rebases_joints() {
        let cycle = tiny_cycle();
        let f0 = &cycle.frames[0];
        let plant = ScenePoint::new(0.55, 0.9, 0.1);
        let planted = cycle.with_plant(plant);
        assert_abs_diff_eq!(planted.frames[0].x, 0.55, epsilon = 1e-9);
        assert_abs_diff_eq!(planted.frames[0].y, 0.9, epsilon = 1e-9);
        let j0 = f0.joints2d.as_ref().unwrap()[0];
        let jp = planted.frames[0].joints2d.as_ref().unwrap()[0];
        assert_abs_diff_eq!(jp[0], j0[0] - f0.x + 0.55, epsilon = 1e-6);
        assert_abs_diff_eq!(jp[1], j0[1] - f0.y + 0.9, epsilon = 1e-6);
    }

    #[test]
    fn hold_idle_is_single_frame() {
        let cycle = tiny_cycle();
        let plant = ScenePoint::new(0.4, 0.85, 0.15);
        let idle = cycle.hold_idle_at(plant);
        assert_eq!(idle.frame_count(), 1);
        assert_eq!(idle.text, "idle");
        assert_abs_diff_eq!(idle.frames[0].x, plant.x, epsilon = 1e-9);
        let stance = cycle.idle_stance_frame_index();
        assert!(stance < cycle.frame_count());
        let idle_si = idle.frames[0].sprite_index.unwrap_or(stance as i32);
        assert_eq!(idle_si, stance as i32);
    }

    #[test]
    fn idle_stance_picks_smaller_foot_span() {
        let cycle = tiny_cycle();
        // frame 0: feet at y 0.9 and 0.91, span ~0.04; frame 1: wider x span
        // frame 0 has smaller span squared
        assert_eq!(cycle.idle_stance_frame_index(), 0);
    }

    #[test]
    fn parse_minimal_json() {
        let raw = r#"{"fps":20,"bones":[[0,1]],"frames":[{"x":0.1,"y":0.2,"depth":0.8}]}"#;
        let m = ArdyMotion::parse(raw).unwrap();
        assert_eq!(m.frame_count(), 1);
        assert_abs_diff_eq!(m.frames[0].x, 0.1, epsilon = 1e-12);
        assert_eq!(m.bones, vec![(0, 1)]);
    }

    #[test]
    fn play_mode_from_string() {
        assert_eq!(ardy_play_mode_from_string(Some("path")), ArdyPlayMode::Path);
        assert_eq!(
            ardy_play_mode_from_string(Some("skeleton")),
            ArdyPlayMode::Skeleton
        );
        assert_eq!(ardy_play_mode_from_string(None), ArdyPlayMode::Both);
        assert_eq!(ardy_play_mode_from_string(Some("nope")), ArdyPlayMode::Both);
    }
}
