//! Gameplay sound triggers (SK-022): physical state transitions become sound events.
//! Pure mapping in `transition_events` so it is testable without the game.
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

/// The sound events (events.json keys) for a state change: leaving a grind plays its exit
/// before a pop or landing.
pub(crate) fn transition_events(from: PhysicalStateId, to: PhysicalStateId) -> Vec<&'static str> {
    let mut events = Vec::new();
    if from == to {
        return events;
    }
    if grind(from) && !grind(to) {
        events.push("grind_end");
    }
    if to == PhysicalStateId::WipeoutGround {
        events.push("bail");
    } else if grind(to) && !grind(from) {
        events.push("grind_start");
    } else if board_air(to) && (on_board_ground(from) || grind(from)) {
        events.push("pop");
    } else if on_board_ground(to) && board_air(from) {
        events.push("land");
    }
    events
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
            for event in transition_events(from, current) {
                sounds.write(PlaySound(event.into()));
            }
        }
    }
}

/// Footstep cadence on foot (SK-042): steps per second at `speed` m/s, None when standing.
/// The original's walk trace has a step every ~0.35-0.5 s; the cadence itself is a guess.
pub(crate) fn step_rate(speed: f32) -> Option<f32> {
    (speed >= 0.3).then(|| (1.2 + 0.45 * speed).clamp(1.6, 3.0))
}

/// A board impact (SK-042): the board loses more than this much ground speed in one frame
/// while rolling, e.g. riding into a wall, a bin or a rail.
const IMPACT_SPEED_DROP: f32 = 2.5;

/// Footsteps while walking and board impacts while rolling.
pub(super) fn motion_sounds(
    skater: Option<Res<crate::physics::SkaterRuntime>>,
    physics: Option<Res<crate::physics::GamePhysics>>,
    replay: Res<crate::replay::Replay>,
    menu: Option<Res<crate::graphics_menu::Menu>>,
    time: Res<Time>,
    mut step_phase: Local<f32>,
    mut last_board_speed: Local<Option<f32>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let (Some(skater), Some(physics)) = (skater, physics) else { return };
    if replay.active || !crate::graphics_menu::gameplay_active(menu) {
        *last_board_speed = None;
        return;
    }
    let state = skater.player_state.current();
    let reckoning = skater.player_input.physical.reckoning.vector_16.map(f32::from_bits);
    let foot_speed = (reckoning[0] * reckoning[0] + reckoning[2] * reckoning[2]).sqrt();
    match (state == PhysicalStateId::BipedGround).then(|| step_rate(foot_speed)).flatten() {
        Some(rate) => {
            *step_phase += rate * time.delta_secs();
            if *step_phase >= 1.0 {
                *step_phase -= 1.0;
                sounds.write(PlaySound("footstep".into()));
            }
        }
        // The first step lands soon after starting to walk.
        None => *step_phase = 0.7,
    }
    let v = physics.board.bodies()[0].rates.linear_velocity;
    let board_speed = (v.x * v.x + v.z * v.z).sqrt();
    let rolling = on_board_ground(state);
    if let (true, Some(last)) = (rolling, *last_board_speed) {
        if last - board_speed > IMPACT_SPEED_DROP {
            sounds.write(PlaySound("impact".into()));
        }
    }
    *last_board_speed = rolling.then_some(board_speed);
}

/// Loop volume and pitch for the board rolling at `speed` m/s (SK-023): silent below
/// 0.5 m/s, full at 10 m/s; pitch rises gently with speed.
pub(crate) fn rolling_mix(speed: f32) -> Option<(f32, f32)> {
    (speed >= 0.5).then(|| {
        let t = ((speed - 0.5) / 9.5).clamp(0.0, 1.0);
        (0.15 + 0.85 * t, 0.85 + 0.35 * t)
    })
}

/// Wind on the board (SK-023, sense_of_speed like the original): silent below 4 m/s, full at
/// 14 m/s.
pub(crate) fn wind_mix(speed: f32) -> Option<f32> {
    (speed >= 4.0).then(|| ((speed - 4.0) / 10.0).clamp(0.0, 1.0))
}

/// Continuous loops: rolling on the board, wind, grinding.
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
        // The original layers four GRINDS loops on a rail (trace SK-042).
        let pitch = 0.9 + (speed / 20.0).min(0.3);
        for key in ["grind_loop", "grind_loop_2", "grind_loop_3", "grind_loop_4"] {
            loops.set(key, key, 0.8, pitch);
        }
    }
    // The original plays two sense_of_speed layers together.
    if on_board_ground(state) || board_air(state) || grind(state) {
        if let Some(volume) = wind_mix(speed) {
            loops.set("wind", "wind", volume, 1.0);
            loops.set("wind_2", "wind_2", volume, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use PhysicalStateId::*;

    #[test]
    fn transitions_map_to_events() {
        assert_eq!(transition_events(PhysicsGround, PhysicsAir), ["pop"]);
        assert_eq!(transition_events(GroundAnimation, KnownAir), ["pop"]);
        assert_eq!(transition_events(KnownAir, PhysicsGround), ["land"]);
        assert_eq!(transition_events(KnownAir, WipeoutGround), ["bail"]);
        assert_eq!(transition_events(KnownAir, GrindFiftyFifty), ["grind_start"]);
        assert_eq!(transition_events(GrindFiftyFifty, KnownAir), ["grind_end", "pop"]);
        assert_eq!(transition_events(GrindFiveO, WipeoutGround), ["grind_end", "bail"]);
        assert!(transition_events(GrindFiftyFifty, GrindBoardslide).is_empty());
        assert!(transition_events(PhysicsAir, KnownAir).is_empty());
    }

    #[test]
    fn footsteps_only_while_moving_on_foot() {
        assert_eq!(step_rate(0.1), None);
        assert!(step_rate(1.5).unwrap() < step_rate(5.0).unwrap());
        assert_eq!(step_rate(20.0), Some(3.0));
    }

    #[test]
    fn rolling_is_silent_when_stopped_and_rises_with_speed() {
        assert_eq!(rolling_mix(0.2), None);
        let (slow, slow_pitch) = rolling_mix(1.0).unwrap();
        let (fast, fast_pitch) = rolling_mix(12.0).unwrap();
        assert!(slow < fast && slow_pitch < fast_pitch);
        assert_eq!((fast, fast_pitch), (1.0, 1.2));
    }

    #[test]
    fn wind_starts_at_speed() {
        assert_eq!(wind_mix(3.0), None);
        assert_eq!(wind_mix(4.0), Some(0.0));
        assert_eq!(wind_mix(30.0), Some(1.0));
    }
}
