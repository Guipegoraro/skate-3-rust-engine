//! Gameplay sound triggers (SK-022): physical state transitions become sound events.
//! Pure mapping in `transition_event` so it is testable without the game.
use super::PlaySound;
use bevy::prelude::*;
use skate_core::player::state::PhysicalStateId;

fn on_board_ground(s: PhysicalStateId) -> bool {
    matches!(s as u32, 100..=103)
}
fn board_air(s: PhysicalStateId) -> bool {
    matches!(s as u32, 200..=202)
}
fn grind(s: PhysicalStateId) -> bool {
    matches!(s as u32, 400..=499)
}

/// The sound event (events.json key) for a state change, if any.
pub(crate) fn transition_event(from: PhysicalStateId, to: PhysicalStateId) -> Option<&'static str> {
    if to == PhysicalStateId::WipeoutGround && from != to {
        return Some("bail");
    }
    if grind(to) && !grind(from) {
        return Some("grind_start");
    }
    if board_air(to) && (on_board_ground(from) || grind(from)) {
        return Some("pop");
    }
    if on_board_ground(to) && board_air(from) {
        return Some("land");
    }
    None
}

pub(super) fn state_sounds(
    skater: Option<Res<crate::physics::SkaterRuntime>>,
    replay: Res<crate::replay::Replay>,
    mut previous: Local<Option<PhysicalStateId>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let Some(skater) = skater else { return };
    let current = skater.player_state.current();
    if let Some(from) = previous.replace(current) {
        if !replay.active {
            if let Some(event) = transition_event(from, current) {
                sounds.write(PlaySound(event.into()));
            }
        }
    }
}

/// Loop volume and pitch for the board rolling at `speed` m/s (SK-023): silent below
/// 0.5 m/s, full at 10 m/s; pitch rises gently with speed.
pub(crate) fn rolling_mix(speed: f32) -> Option<(f32, f32)> {
    (speed >= 0.5).then(|| {
        let t = ((speed - 0.5) / 9.5).clamp(0.0, 1.0);
        (0.15 + 0.85 * t, 0.85 + 0.35 * t)
    })
}

/// Continuous loops: rolling on the board, grinding.
pub(super) fn loop_sounds(
    skater: Option<Res<crate::physics::SkaterRuntime>>,
    physics: Option<Res<crate::physics::GamePhysics>>,
    replay: Res<crate::replay::Replay>,
    menu: Option<Res<crate::graphics_menu::Menu>>,
    mut loops: ResMut<super::SoundLoops>,
) {
    let (Some(skater), Some(physics)) = (skater, physics) else { return };
    if replay.active || !crate::graphics_menu::gameplay_active(menu) {
        return;
    }
    let state = skater.player_state.current();
    let v = physics.board.bodies()[0].rates.linear_velocity;
    let speed = (v.x * v.x + v.z * v.z).sqrt();
    if on_board_ground(state) {
        if let Some((volume, pitch)) = rolling_mix(speed) {
            loops.set("rolling", "rolling", volume, pitch);
        }
    }
    if grind(state) {
        loops.set("grind", "grind_loop", 0.8, 0.9 + (speed / 20.0).min(0.3));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PhysicalStateId::*;

    #[test]
    fn transitions_map_to_events() {
        assert_eq!(transition_event(PhysicsGround, PhysicsAir), Some("pop"));
        assert_eq!(transition_event(GroundAnimation, KnownAir), Some("pop"));
        assert_eq!(transition_event(KnownAir, PhysicsGround), Some("land"));
        assert_eq!(transition_event(KnownAir, WipeoutGround), Some("bail"));
        assert_eq!(transition_event(KnownAir, GrindFiftyFifty), Some("grind_start"));
        assert_eq!(transition_event(GrindFiftyFifty, GrindBoardslide), None);
        assert_eq!(transition_event(PhysicsAir, KnownAir), None);
    }

    #[test]
    fn rolling_is_silent_when_stopped_and_rises_with_speed() {
        assert_eq!(rolling_mix(0.2), None);
        let (slow, slow_pitch) = rolling_mix(1.0).unwrap();
        let (fast, fast_pitch) = rolling_mix(12.0).unwrap();
        assert!(slow < fast && slow_pitch < fast_pitch);
        assert_eq!((fast, fast_pitch), (1.0, 1.2));
    }
}
