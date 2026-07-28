//! ECS systems: walker tick + click-to-walk dispatch.
//!
//! The engine runs these systems every frame:
//!   * [`walker_tick_system`] — advance `Walker` plants along their route,
//!     update `Transform2D` pixel position.
//!   * [`click_to_walk_system`] — on a click event, find the path from
//!     the player's current position to the clicked target, install
//!     a new route on the walker.
//!
//! The split mirrors UE's `UCharacterMovementComponent` (per-tick
//! advance) vs `APlayerController` (input → intent).

use std::time::Duration;

use adventure_core::math::Vec2;
use adventure_locomotion::WalkGraph;
use bevy_ecs::prelude::*;

use crate::components::{Player, Transform2D, Walker};

/// Frame context passed to systems. Carries wall-clock dt + the
/// (logical) window size in pixels for normalized → pixel projection.
#[derive(Resource, Debug, Clone, Copy)]
pub struct FrameContext {
    /// Wall-clock seconds since last frame.
    pub dt: f32,
    /// Logical window width in pixels.
    pub width: f32,
    /// Logical window height in pixels.
    pub height: f32,
}

impl Default for FrameContext {
    fn default() -> Self {
        Self {
            dt: 1.0 / 60.0,
            width: 800.0,
            height: 600.0,
        }
    }
}

/// Pending click in normalized scene coords (0..1 per axis).
///
/// Set by the input layer; cleared after dispatch.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PendingClick {
    /// Click position in normalized room coords.
    pub target: Option<Vec2>,
}

/// Resource holding the active walk graph (built from the scene).
#[derive(Resource, Debug, Clone)]
pub struct SceneGraph {
    /// The locomotion walk graph.
    pub graph: WalkGraph,
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self {
            graph: WalkGraph {
                nodes: vec![],
                edges: vec![],
            },
        }
    }
}

impl SceneGraph {
    /// Empty graph (no nodes).
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the graph with the supplied one.
    pub fn set(&mut self, g: WalkGraph) {
        self.graph = g;
    }
}

/// Convert a normalized scene point to pixel space.
pub fn scene_to_pixel(p: adventure_locomotion::ScenePoint, w: f32, h: f32) -> Vec2 {
    Vec2::new((p.x as f32) * w, (p.y as f32) * h)
}

/// Convert a pixel-space point to a normalized scene point at the
/// given depth (default 0.5).
pub fn pixel_to_scene(p: Vec2, w: f32, h: f32, depth: f64) -> adventure_locomotion::ScenePoint {
    adventure_locomotion::ScenePoint::new((p.x / w) as f64, (p.y / h) as f64, depth)
}

/// System: advance every walker along its route by `FrameContext::dt`.
///
/// Walkers without an active plant or with an empty route are skipped.
/// On arrival (cursor reaches end), the plant is cleared and the route
/// is dropped.
pub fn walker_tick_system(
    frame: Res<FrameContext>,
    mut q: Query<(&mut Walker, &mut Transform2D)>,
) {
    for (mut walker, mut transform) in q.iter_mut() {
        if !walker.is_walking() {
            continue;
        }
        // Advance plant wall-clock.
        if let Some(plant) = walker.plant.as_mut() {
            plant.elapsed_sec += frame.dt as f64;
        }

        // Advance along the route proportionally to dt * speed.
        let mut remaining = (walker.speed as f64) * (frame.dt as f64);

        let mut cur = pixel_to_scene(
            transform.pos,
            frame.width,
            frame.height,
            0.0,
        );

        while remaining > 0.0 && walker.route_cursor < walker.route.len() {
            let target = walker.route[walker.route_cursor];
            let seg = target.distance_xy(cur);
            if seg <= remaining {
                cur = target;
                walker.route_cursor += 1;
                remaining -= seg;
            } else {
                let t = remaining / seg.max(f64::EPSILON);
                cur = adventure_locomotion::ScenePoint::lerp(cur, target, t);
                remaining = 0.0;
            }
        }

        transform.pos = scene_to_pixel(cur, frame.width, frame.height);

        // Update facing based on direction of motion (before arrival check).
        if !walker.route.is_empty() {
            // Use the very next unvisited hop or the last visited hop.
            let idx = walker.route_cursor.min(walker.route.len() - 1);
            let target = walker.route[idx];
            let dx = (target.x - cur.x) as f32;
            if dx.abs() > 1e-4 {
                use adventure_scene::transform::Facing;
                transform.facing = if dx < 0.0 { Facing::West } else { Facing::East };
            }
        }

        if walker.route_cursor >= walker.route.len() {
            walker.stop();
        }
    }
}

/// System: handle the pending click. If a click is pending and the
/// graph has a path, install a route on the player walker.
///
/// The click is consumed (cleared) regardless of whether a path was
/// found.
pub fn click_to_walk_system(
    mut pending: ResMut<PendingClick>,
    graph: Res<SceneGraph>,
    mut q: Query<(&mut Walker, &mut Transform2D), With<Player>>,
) {
    let target = match pending.target.take() {
        Some(t) => t,
        None => return,
    };

    let Ok((mut walker, mut transform)) = q.get_single_mut() else {
        return;
    };

    let from = pixel_to_scene(transform.pos, 800.0, 600.0, 0.0);
    let to = adventure_locomotion::ScenePoint::new(target.x as f64, target.y as f64, 0.0);

    // Snapshot the visual name before mutable borrow of walker.
    let from_visual = walker.current_visual.clone();

    let facing = if graph.graph.is_empty() {
        walker.begin_walk(from, vec![to], from_visual.as_deref(), "walk_target")
    } else {
        let path = graph.graph.find_path(from, to);
        if path.is_empty() {
            walker.begin_walk(from, vec![to], from_visual.as_deref(), "walk_target")
        } else {
            walker.begin_walk(from, path, from_visual.as_deref(), "walk_target")
        }
    };

    if let Some(facing) = facing {
        transform.facing = facing;
    }
}

/// Schedule: run click_to_walk first (installs route), then walker_tick.
pub struct FrameSchedule {
    schedule: Schedule,
}

impl Default for FrameSchedule {
    fn default() -> Self {
        let mut schedule = Schedule::default();
        schedule.add_systems((click_to_walk_system, walker_tick_system).chain());
        Self { schedule }
    }
}

impl FrameSchedule {
    /// Run a single frame: updates FrameContext then runs the schedule.
    pub fn run(
        &mut self,
        world: &mut World,
        dt: Duration,
        width: f32,
        height: f32,
    ) {
        {
            let mut frame = world.resource_mut::<FrameContext>();
            frame.dt = dt.as_secs_f32();
            frame.width = width;
            frame.height = height;
        }
        self.schedule.run(world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adventure_locomotion::{ArdyFrame, ArdyMotion};
    #[allow(unused_imports)]
    use adventure_locomotion::ScenePoint;

    fn empty_cycle() -> ArdyMotion {
        ArdyMotion::new(20.0, vec![ArdyFrame::new(0.5, 0.5, 0.5)], vec![])
    }

    fn make_world_with_player() -> World {
        let mut world = World::new();
        world.insert_resource(FrameContext::default());
        world.insert_resource(PendingClick::default());
        world.insert_resource(SceneGraph::new());
        world.spawn((
            Player,
            Walker::idle(empty_cycle()),
            Transform2D::at(Vec2::new(400.0, 300.0)),
        ));
        world
    }

    #[test]
    fn tick_advances_idle_walker_no_op() {
        let mut world = make_world_with_player();
        let mut sched = FrameSchedule::default();
        sched.run(&mut world, Duration::from_secs_f32(0.016), 800.0, 600.0);
        let mut q = world.query::<(&Walker, &Transform2D)>();
        let (w, t) = q.iter(&world).next().unwrap();
        assert!(!w.is_walking());
        assert_eq!(t.pos, Vec2::new(400.0, 300.0));
    }

    #[test]
    fn click_installs_route_on_player() {
        let mut world = make_world_with_player();
        {
            let mut pending = world.resource_mut::<PendingClick>();
            pending.target = Some(Vec2::new(0.9, 0.9));
        }
        let mut sched = FrameSchedule::default();
        sched.run(&mut world, Duration::from_secs_f32(0.0), 800.0, 600.0);
        let mut q = world.query::<&Walker>();
        let w = q.iter(&world).next().unwrap();
        assert!(!w.route.is_empty() || w.plant.is_some());
    }

    #[test]
    fn tick_advances_walker_along_route() {
        let mut world = make_world_with_player();

        {
            let mut q = world.query::<(&mut Walker, &mut Transform2D)>();
            let (mut w, mut t) = q.iter_mut(&mut world).next().unwrap();
            let from = adventure_locomotion::ScenePoint::new(0.5, 0.5, 0.0);
            let to = adventure_locomotion::ScenePoint::new(0.9, 0.5, 0.0);
            let facing = w.begin_walk(from, vec![to], None, "walk_e");
            t.facing = facing.unwrap_or(t.facing);
        }

        let mut sched = FrameSchedule::default();
        sched.run(&mut world, Duration::from_secs(10), 800.0, 600.0);
        let mut q = world.query::<(&Walker, &Transform2D)>();
        let (w, t) = q.iter(&world).next().unwrap();
        assert!(!w.is_walking(), "walker should have stopped on arrival");
        assert!((t.pos.x - 720.0).abs() < 1.0, "x was {}", t.pos.x);
    }

    #[test]
    fn walker_faces_direction_of_motion() {
        let mut world = make_world_with_player();
        {
            let mut q = world.query::<(&mut Walker, &mut Transform2D)>();
            let (mut w, mut t) = q.iter_mut(&mut world).next().unwrap();
            let from = adventure_locomotion::ScenePoint::new(0.5, 0.5, 0.0);
            let to = adventure_locomotion::ScenePoint::new(0.1, 0.5, 0.0);
            let facing = w.begin_walk(from, vec![to], None, "walk_w");
            t.facing = facing.unwrap_or(t.facing);
        }
        let mut sched = FrameSchedule::default();
        sched.run(&mut world, Duration::from_secs(2), 800.0, 600.0);
        let mut q = world.query::<&Transform2D>();
        let t = q.iter(&world).next().unwrap();
        use adventure_scene::transform::Facing;
        assert_eq!(t.facing, Facing::West, "walker should face West");
    }

    #[test]
    fn scene_to_pixel_projects_correctly() {
        let p = adventure_locomotion::ScenePoint::new(0.5, 0.5, 0.0);
        let px = scene_to_pixel(p, 800.0, 600.0);
        assert_eq!(px, Vec2::new(400.0, 300.0));
    }

    #[test]
    fn pixel_to_scene_inverse() {
        let px = Vec2::new(400.0, 300.0);
        let s = pixel_to_scene(px, 800.0, 600.0, 0.5);
        assert!((s.x - 0.5).abs() < 1e-4);
        assert!((s.y - 0.5).abs() < 1e-4);
    }

    #[test]
    fn frame_schedule_runs_chain() {
        // Click + tick in same frame: walker should be en-route.
        let mut world = make_world_with_player();
        {
            let mut pending = world.resource_mut::<PendingClick>();
            pending.target = Some(Vec2::new(0.9, 0.5));
        }
        let mut sched = FrameSchedule::default();
        // Tiny dt — walker should be partway, not arrived.
        sched.run(&mut world, Duration::from_millis(50), 800.0, 600.0);
        let mut q = world.query::<(&Walker, &Transform2D)>();
        let (w, t) = q.iter(&world).next().unwrap();
        let _ = (w, t);
        // We don't assert arrival; just that it ran without panic.
    }
}
