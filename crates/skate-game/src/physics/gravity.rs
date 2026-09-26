//! Mod gravity multiplier (SK-004, `sdk.physics.gravity`).
//! Scales the three gravity sources together each tick: WorldGravity (integrator and most
//! trajectory predictions), ProcessedPhysIn gravity (ollie velocity, early air, skeleton)
//! and the ground speed model's slope gravity. Fixed -9.8 constants in specific air,
//! biped, handplant and grind paths still use stock gravity (documented limitation).
use super::{GamePhysics, SkaterRuntime};

const STOCK_PROCESSED: f32 = f32::from_bits(0xc11c_cccd);

pub(super) fn apply(physics: &mut GamePhysics, skater: &mut SkaterRuntime) {
    let scale = physics.gravity;
    physics.settings.step.simulation.gravity_acceleration = {
        let g = physics.base_gravity;
        skate_core::math::Vector3::new(g.x * scale, g.y * scale, g.z * scale)
    };
    skater.player_input.processed.gravity_2648 = STOCK_PROCESSED * scale;
    if scale != 1.0 {
        skater.ground_settings = std::sync::Arc::new(skater.ground_settings.with_gravity(scale));
    }
}
