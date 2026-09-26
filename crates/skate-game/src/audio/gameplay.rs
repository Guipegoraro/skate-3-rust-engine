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
}
