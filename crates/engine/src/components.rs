//! ECS components for the engine world.
//!
//! Components map the authored [`adventure_scene`] data into
//! `bevy_ecs`-style runtime state. The main loop queries them via
//! systems in [`crate::systems`].

use adventure_core::math::Vec2;
use adventure_locomotion::{ArdyMotion, PlantController, ScenePoint};
use adventure_scene::transform::Facing;
use bevy_ecs::prelude::*;

/// Pixel-space position + facing of a visible entity.
///
/// Mirrors `adventure_scene::transform::Transform2D` but kept here
/// as a runtime ECS component so we can mutate it directly during
/// the tick loop.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Transform2D {
    /// Position in pixel space.
    pub pos: Vec2,
    /// Rotation in radians (rarely used for walkers; mainly for props).
    pub rot: f32,
    /// Scale (1, 1 = native).
    pub scale: Vec2,
    /// Which way the walker is facing.
    pub facing: Facing,
}

impl Transform2D {
    /// Construct at `pos` with default scale + facing East.
    pub fn at(pos: Vec2) -> Self {
        Self {
            pos,
            rot: 0.0,
            scale: Vec2::ONE,
            facing: Facing::East,
        }
    }
}

/// A walker entity. Holds the active plant + the locomotion cycle
/// (frames to play while walking) + the pending route.
#[derive(Component, Debug)]
pub struct Walker {
    /// Current locomotion cycle (e.g. an ardy_walk_demo.json).
    pub cycle: ArdyMotion,
    /// Active plant controller; None when idle.
    pub plant: Option<PlantController>,
    /// Pending route as a sequence of normalized scene points.
    /// Tick advances the walker along this polyline.
    pub route: Vec<ScenePoint>,
    /// Index into `route` of the next target. Equals `route.len()` when done.
    pub route_cursor: usize,
    /// Walk speed in normalized units per second (typical 0.3).
    pub speed: f32,
    /// Visual state name (used to look up the appropriate cycle + mirror).
    pub current_visual: Option<String>,
}

impl Walker {
    /// Idle walker with a cycle but no active plant/route.
    pub fn idle(cycle: ArdyMotion) -> Self {
        Self {
            cycle,
            plant: None,
            route: Vec::new(),
            route_cursor: 0,
            speed: 0.3,
            current_visual: None,
        }
    }

    /// Is this walker currently moving?
    pub fn is_walking(&self) -> bool {
        self.plant.is_some() && self.route_cursor < self.route.len()
    }

    /// Begin a walk along `route` from `from`.
    ///
    /// Replaces any current route. The plant FSM is started with the
    /// total route distance so stride quantisation works. The
    /// `initial_facing` is the facing to apply on this same call,
    /// derived from the first hop's direction.
    pub fn begin_walk(
        &mut self,
        from: ScenePoint,
        route: Vec<ScenePoint>,
        from_visual: Option<&str>,
        to_visual: &str,
    ) -> Option<adventure_scene::transform::Facing> {
        // Compute total polyline length (in normalized space) for plant timing.
        let total = route
            .iter()
            .fold((from, 0.0f64), |(prev, acc), p| {
                (*p, acc + prev.distance_xy(*p))
            })
            .1;
        self.plant = Some(PlantController::start(from_visual, to_visual, total));
        // Compute facing from the first hop (or stay as-is if route empty).
        let facing = route.first().map(|p| {
            let dx = (p.x - from.x) as f32;
            if dx < 0.0 {
                adventure_scene::transform::Facing::West
            } else {
                adventure_scene::transform::Facing::East
            }
        });
        self.route = route;
        self.route_cursor = 0;
        self.current_visual = Some(to_visual.to_string());
        facing
    }

    /// Clear the active walk (e.g. on arrival).
    pub fn stop(&mut self) {
        self.plant = None;
        self.route.clear();
        self.route_cursor = 0;
    }
}

/// Tag: this walker is the player-controlled one (click-to-walk target).
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Player;

/// Optional camera offset for rooms larger than the screen.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct CameraOffset {
    /// Pixel offset.
    pub offset: Vec2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_default_at_origin() {
        let t = Transform2D::default();
        assert_eq!(t.pos, Vec2::ZERO);
    }

    #[test]
    fn transform_at_constructor() {
        let t = Transform2D::at(Vec2::new(10.0, 20.0));
        assert_eq!(t.pos, Vec2::new(10.0, 20.0));
        assert_eq!(t.scale, Vec2::ONE);
        assert_eq!(t.facing, Facing::East);
    }

    fn empty_cycle() -> ArdyMotion {
        ArdyMotion::new(20.0, vec![], vec![])
    }

    #[test]
    fn walker_idle_not_walking() {
        let w = Walker::idle(empty_cycle());
        assert!(!w.is_walking());
    }

    #[test]
    fn walker_begin_then_stop() {
        let mut w = Walker::idle(empty_cycle());
        let from = ScenePoint::new(0.0, 0.0, 0.0);
        let to = ScenePoint::new(0.5, 0.5, 0.5);
        let facing = w.begin_walk(from, vec![to], None, "walk_se");
        // East-bound since dx > 0.
        assert_eq!(facing, Some(adventure_scene::transform::Facing::East));
        assert!(w.plant.is_some());
        assert_eq!(w.route.len(), 1);
        assert!(w.is_walking() || w.route_cursor >= w.route.len());
        w.stop();
        assert!(!w.is_walking());
        assert!(w.route.is_empty());
    }

    #[test]
    fn player_tag_default() {
        let _ = Player::default();
    }
}
