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

/// A foot swinging faster than this (m/s) is off the ground; one slower than
/// `FOOT_PLANTED` after a swing has just landed (SK-061).
const FOOT_SWINGING: f32 = 1.2;
/// Left and right foot parts of the physical skeleton record (as the camera reads them).
pub(crate) const FOOT_PARTS: [usize; 2] = [15, 19];
const FOOT_PLANTED: f32 = 0.4;

/// Foot plants from the physical skeleton's foot positions (SK-061): a planted foot stands
/// still while the body walks over it, so a foot that swings and then stops is a step.
/// Speed-based, so slopes and stairs need no ground height. Feed it once per physics tick.
#[derive(Clone, Debug, Default)]
pub(crate) struct FootPlants {
    last: Option<[[f32; 3]; 2]>,
    swinging: [bool; 2],
}
impl FootPlants {
    /// Feet positions after a tick of `dt` s; returns how many feet planted.
    pub fn update(&mut self, feet: [[f32; 3]; 2], dt: f32) -> u32 {
        let Some(last) = self.last.replace(feet) else { return 0 };
        let mut plants = 0;
        for foot in 0..2 {
            let d: Vec<f32> = (0..3).map(|i| feet[foot][i] - last[foot][i]).collect();
            let speed = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() / dt.max(1e-4);
            if speed > FOOT_SWINGING {
                self.swinging[foot] = true;
            } else if speed < FOOT_PLANTED && self.swinging[foot] {
                self.swinging[foot] = false;
                plants += 1;
            }
        }
        plants
    }
    /// Standing, riding or teleporting: start over without a step.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Whether a footstep plays this frame (SK-061). Steps come from foot plants (`marked`: the
/// skeleton's feet landing, or the AudibleFootStepStrength marks of the stock OffBoard graph,
/// which only its step-off clips carry); the speed cadence (`rate`, None when not walking)
/// only fills in after 1.5 periods without a plant. `phase` counts periods since the last step.
pub(crate) fn footstep_due(marked: bool, rate: Option<f32>, dt: f32, phase: &mut f32) -> bool {
    if marked {
        *phase = 0.0;
        return true;
    }
    let Some(rate) = rate else {
        // The first cadence step lands soon after starting to walk.
        *phase = 0.8;
        return false;
    };
    *phase += rate * dt;
    if *phase >= 1.5 {
        *phase -= 1.0;
        return true;
    }
    false
}

/// A board impact (SK-042): the board loses more than this much ground speed in one frame
/// while rolling, e.g. riding into a wall, a bin or a rail.
const IMPACT_SPEED_DROP: f32 = 2.5;

/// Impact sound for the hit surface (SK-061). `packed_surface` is the triangle's authored
/// code; its low 7 bits are the original's audio surface list (names in the vendored
/// University Blender addon, `_AUDIO_NAMES`): Metal 9, the rail/pipe/sheet/complex metals
/// 11-40, Manhole_Metal and the grates 67-69, OilDrum 84, DMORail 85, Metal_Rail_4 89 and
/// Metal_Ramp 91 sound like Skate_Metal/47-49 (board hitting a rail, SK-042 trace); anything
/// else, or no surface, keeps the concrete hit.
pub(crate) fn impact_event(packed_surface: Option<u16>) -> &'static str {
    match packed_surface.map(|s| s & 0x7f) {
        Some(9 | 11..=40 | 67..=69 | 84 | 85 | 89 | 91) => "impact_metal",
        _ => "impact",
    }
}

/// Where to look for what the board hit: from the deck along last frame's ground velocity,
/// a little past a board length. None when it was not moving sideways.
pub(crate) fn impact_probe(position: [f32; 3], velocity: [f32; 3]) -> Option<([f32; 3], [f32; 3])> {
    let speed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
    (speed > 0.5).then(|| {
        let reach = 1.0 / speed;
        let end = [position[0] + velocity[0] * reach, position[1], position[2] + velocity[2] * reach];
        (position, end)
    })
}

/// Packed surface of whatever the board ran into, from the probe above.
fn impact_surface(physics: &crate::physics::GamePhysics, position: [f32; 3], velocity: [f32; 3]) -> Option<u16> {
    use skate_core::math::Vector3;
    let (start, end) = impact_probe(position, velocity)?;
    let world = physics.world();
    let hit = world.query_swept_line(Vector3::new(start[0], start[1], start[2]), Vector3::new(end[0], end[1], end[2]), 0.15).ok()??;
    world.packed_surface(hit.triangle)
}

/// Footsteps while walking and board impacts while rolling.
pub(super) fn motion_sounds(
    skater: Option<Res<crate::physics::SkaterRuntime>>,
    physics: Option<Res<crate::physics::GamePhysics>>,
    replay: Res<crate::replay::Replay>,
    menu: Option<Res<crate::graphics_menu::Menu>>,
    time: Res<Time>,
    mut step_phase: Local<f32>,
    mut marked_steps: Local<Option<u32>>,
    mut feet: Local<(FootPlants, u64)>,
    mut last_board: Local<Option<([f32; 3], [f32; 3])>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let (Some(skater), Some(physics)) = (skater, physics) else { return };
    if replay.active || !crate::graphics_menu::gameplay_active(menu) {
        *last_board = None;
        return;
    }
    let state = skater.player_state.current();
    let reckoning = skater.player_input.physical.reckoning.vector_16.map(f32::from_bits);
    let foot_speed = (reckoning[0] * reckoning[0] + reckoning[2] * reckoning[2]).sqrt();
    let rate = (state == PhysicalStateId::BipedGround).then(|| step_rate(foot_speed)).flatten();
    let steps = skater.animation_input.footsteps.count;
    let mut marked = marked_steps.replace(steps).is_some_and(|last| last != steps);
    // Feet are sampled once per physics tick (render frames can outnumber ticks).
    if state == PhysicalStateId::BipedGround {
        let ticks = physics.ticks.saturating_sub(feet.1);
        if ticks > 0 {
            let pose = &skater.skeleton.record.pose;
            let foot = |i: usize| [pose[i][3][0], pose[i][3][1], pose[i][3][2]];
            marked |= feet.0.update([foot(FOOT_PARTS[0]), foot(FOOT_PARTS[1])], ticks as f32 / 60.0) > 0;
        }
    } else {
        feet.0.reset();
    }
    feet.1 = physics.ticks;
    if footstep_due(marked, rate, time.delta_secs(), &mut step_phase) {
        sounds.write(PlaySound("footstep".into()));
    }
    let deck = &physics.board.bodies()[0].rates;
    let (p, v) = (deck.position, deck.linear_velocity);
    let board_speed = (v.x * v.x + v.z * v.z).sqrt();
    let rolling = on_board_ground(state);
    if let (true, Some((position, velocity))) = (rolling, *last_board) {
        let last = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
        if last - board_speed > IMPACT_SPEED_DROP {
            sounds.write(PlaySound(impact_event(impact_surface(&physics, position, velocity)).into()));
        }
    }
    *last_board = rolling.then_some(([p.x, p.y, p.z], [v.x, v.y, v.z]));
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
    fn foot_plants_follow_a_synthetic_walk() {
        // Two feet walking at 1.2 m/s, 60 Hz: each foot stands still for 0.375 s, then swings
        // 0.6 m forward (and 8 cm up and down) in 0.375 s. The feet alternate.
        let dt = 1.0 / 60.0;
        let foot_at = |t: f32, offset: f32| -> [f32; 3] {
            let t = t + offset;
            let (cycle, phase) = ((t / 0.75).floor(), (t / 0.75).fract());
            let swing = ((phase - 0.5) / 0.5 * 2.0).clamp(0.0, 1.0);
            let lift = if swing > 0.0 && swing < 1.0 { 0.08 * (swing * std::f32::consts::PI).sin() } else { 0.0 };
            [0.0, lift, 0.6 * (cycle + swing)]
        };
        let mut plants = FootPlants::default();
        let steps: Vec<usize> = (0..180)
            .filter(|&tick| {
                let t = tick as f32 * dt;
                plants.update([foot_at(t, 0.0), foot_at(t, 0.375)], dt) > 0
            })
            .collect();
        // 3 s at one plant per foot every 0.75 s: 8 steps, alternating every ~22 ticks.
        assert_eq!(steps.len(), 8, "{steps:?}");
        assert!(steps.windows(2).all(|w| (20..=25).contains(&(w[1] - w[0]))), "{steps:?}");
        // Standing still, or a slow shuffle below the swing speed, is not a step.
        let mut plants = FootPlants::default();
        assert!((0..120).all(|tick| plants.update([[0.0; 3], [0.0, 0.0, tick as f32 * 0.01]], dt) == 0));
    }

    #[test]
    fn marked_footsteps_play_and_hold_off_the_cadence() {
        let dt = 1.0 / 60.0;
        let mut phase = 0.0;
        // Marked steps every 0.4 s at a 2/s cadence: only the marked ones play.
        let played: Vec<usize> = (0..120).filter(|&frame| footstep_due(frame % 24 == 0, Some(2.0), dt, &mut phase)).collect();
        assert_eq!(played, [0, 24, 48, 72, 96]);
        // Without markers the cadence fills in: first after 1.5 periods, then every period.
        let mut phase = 0.0;
        let played: Vec<usize> = (0..120).filter(|&frame| footstep_due(false, Some(2.0), dt, &mut phase)).collect();
        assert_eq!(played.len(), 3, "{played:?}");
        assert!((44..=46).contains(&played[0]) && (29..=31).contains(&(played[1] - played[0])), "{played:?}");
        // Standing still: marked steps (landing a jump) still play, the cadence does not.
        assert!(footstep_due(true, None, dt, &mut phase));
        assert!(!(0..600).any(|_| footstep_due(false, None, dt, &mut phase)));
    }

    #[test]
    fn impact_sound_follows_the_hit_surface() {
        // Low 7 bits pick the sound; the physics category above them does not matter.
        assert_eq!(impact_event(Some(11)), "impact_metal", "Metal_Solid_Round_1 rail");
        assert_eq!(impact_event(Some(31 | 4 << 7)), "impact_metal", "Metal_Sheet, slippery");
        assert_eq!(impact_event(Some(85)), "impact_metal", "DMORail");
        assert_eq!(impact_event(Some(3)), "impact", "Concrete_Polished");
        assert_eq!(impact_event(Some(53 | 8 << 7)), "impact", "Concrete_Curb stair");
        assert_eq!(impact_event(Some(6)), "impact", "Wood_Ramp");
        assert_eq!(impact_event(Some(0)), "impact", "test course, material 0");
        assert_eq!(impact_event(None), "impact");
    }

    #[test]
    fn impact_probe_looks_ahead_of_the_board() {
        let (start, end) = impact_probe([1.0, 2.0, 3.0], [0.0, -1.0, 6.0]).unwrap();
        assert_eq!(start, [1.0, 2.0, 3.0]);
        assert!((end[2] - 4.0).abs() < 1e-6 && end[0] == 1.0 && end[1] == 2.0, "{end:?}");
        assert_eq!(impact_probe([0.0; 3], [0.1, -5.0, 0.2]), None);
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
