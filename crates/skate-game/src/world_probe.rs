//! Line queries against the physics collision world (`BoardWorld`) for game code that needs
//! to know where the ground or a wall is without running physics: fly mode landing, NPCs.
use bevy::prelude::*;
use skate_core::{
    math::Vector3,
    physics::{
        board_world::BoardWorld,
        triangle_query::{TriangleLineHit, triangle_segment},
    },
};

/// Nearest hit of the segment `from`→`to` on a triangle whose normal passes `keep`.
pub(crate) fn first_hit(
    world: &BoardWorld,
    from: Vec3,
    to: Vec3,
    keep: impl Fn(Vec3) -> bool,
) -> Option<(Vec3, Vec3)> {
    let start = Vector3::new(from.x, from.y, from.z);
    let end = Vector3::new(to.x, to.y, to.z);
    let delta = Vector3::new(to.x - from.x, to.y - from.y, to.z - from.z);
    let mut best: Option<(f32, Vector3, Vector3)> = None;
    for (_, entry) in world.line_candidates(start, end, 0.0) {
        let n = entry.triangle.feature.normal;
        if !keep(Vec3::new(n.x, n.y, n.z)) {
            continue;
        }
        let mut hit = TriangleLineHit { position: Vector3::ZERO, normal: Vector3::ZERO,
            fraction: 0.0, volume_parameter: [0.0; 3] };
        if triangle_segment(&mut hit, start, delta, entry.triangle.vertices, 0.0, 0.0)
            && best.is_none_or(|(fraction, _, _)| hit.fraction < fraction)
        {
            best = Some((hit.fraction, hit.position, n));
        }
    }
    best.map(|(_, p, n)| (Vec3::new(p.x, p.y, p.z), Vec3::new(n.x, n.y, n.z)))
}

/// Closest surface facing up at least `min_normal_y` within `depth` straight below `position`.
pub(crate) fn ground_below(world: &BoardWorld, position: Vec3, depth: f32, min_normal_y: f32) -> Option<Vec3> {
    first_hit(world, position, position - Vec3::Y * depth, |n| n.y >= min_normal_y).map(|(p, _)| p)
}

/// True when the segment `from`→`to` hits a surface steeper than `max_normal_y` (a wall).
pub(crate) fn wall_between(world: &BoardWorld, from: Vec3, to: Vec3, max_normal_y: f32) -> bool {
    first_hit(world, from, to, |n| n.y.abs() < max_normal_y).is_some()
}
