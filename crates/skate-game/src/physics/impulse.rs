//! Mod velocity impulse (SK-005, `sdk.player.impulse`). Applied after the state update and
//! before the shared solve, to the seven real board bodies and the skeleton together.
//! Only riding states whose solve keeps body velocity accept it; elsewhere the state
//! overwrites velocity (KnownAir, plants, grind, on foot), so the request is dropped.
use super::{GamePhysics, SkaterRuntime};
use skate_core::player::state::PhysicalStateId;

/// Largest velocity change per tick, m/s.
pub(crate) const MAX_IMPULSE: f32 = 20.0;

pub(super) fn apply(physics: &mut GamePhysics, skater: &mut SkaterRuntime) {
    let Some(dv) = physics.pending_impulse.take() else { return };
    use PhysicalStateId::*;
    if !matches!(skater.player_state.current(), PhysicsGround | SlideGround | RevertGround | PhysicsAir) {
        return;
    }
    let add = |v: &mut skate_core::math::Vector3| {
        v.x += dv[0];
        v.y += dv[1];
        v.z += dv[2];
    };
    for body in physics.board.bodies_mut() {
        add(&mut body.rates.linear_velocity);
    }
    for body in skater.skeleton.bodies_mut() {
        add(&mut body.rates.linear_velocity);
    }
}

/// Adds a request to the pending impulse, capping the tick total at MAX_IMPULSE.
pub(crate) fn queue(physics: &mut GamePhysics, velocity: [f32; 3]) {
    let mut total = physics.pending_impulse.unwrap_or_default();
    for i in 0..3 {
        total[i] += velocity[i];
    }
    let length = total.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length > MAX_IMPULSE {
        total = total.map(|v| v * MAX_IMPULSE / length);
    }
    physics.pending_impulse = Some(total);
}
