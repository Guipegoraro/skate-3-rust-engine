//! SK-062: grind snap sweep on the test course's low rail. Rides at a fixed speed toward
//! the rail, ollies so the deck comes down over it, and measures whether and how the
//! grind locks. Needs SKATE3_ASSET_ROOT; run with `--ignored --nocapture grind_snap`.
//! Per-run CSVs go to `$SK062_OUT` (default: the system temp dir `sk062/`).
use super::scripted_play::{Assets, Session};
use super::GamePhysics;
use crate::difficulty::Difficulty;
use skate_core::physics::board::BodyId;

/// Low rail of `grind_world::rails()`: x=-7, z=7..15, top 0.45 m above the lower floor.
const RAIL_X: f32 = -7.0;
const RAIL_Z: [f32; 2] = [7.0, 15.0];
const RAIL_Y: f32 = super::ground::FLOOR_HEIGHT + 0.45;
/// The deck must be up over the rail's near end (z=15) by this margin, or it rides into it.
const END_CLEARANCE: f32 = 0.3;
/// Deck height over the rail top where a 50-50 sits (probe .02 + truck); aim the ollie here.
const DECK_OVER_RAIL: f32 = 0.1;
/// physics_grinds DeckCenterToTruck (disc value, see the card research).
const DECK_TO_TRUCK: f32 = 0.243;
const SETTLE_TICKS: usize = 40;
const RUN_UP: f32 = 9.0;

struct Sample {
    tick: usize,
    state: u32,
    family: i32,
    deck: [f32; 3],
    /// Signed lateral offset (x - rail) of the front and rear truck probe centres.
    trucks: [f32; 2],
    /// Probe centre height over the rail top (probe reaches .2 m below it).
    truck_height: [f32; 2],
    truck_z: [f32; 2],
    yaw: f32,
    along: f32,
    across: f32,
    vy: f32,
    targeting: bool,
    pelvis_x: f32,
}

fn sample(s: &Session) -> Sample {
    let frame = super::solve::deck_frame(&s.physics.board);
    let [_, up, forward, position] = frame;
    let truck = |sign: f32| -> [f32; 3] {
        std::array::from_fn(|i| position[i] - up[i] * 0.02 + forward[i] * DECK_TO_TRUCK * sign)
    };
    let (front, rear) = (truck(1.), truck(-1.));
    let v = s.physics.board.bodies()[BodyId::Deck.index()].rates.linear_velocity;
    // Yaw of the deck against the rail line (z), folded to -90..90 degrees.
    let mut yaw = forward[0].atan2(forward[2]).to_degrees();
    if yaw > 90. { yaw -= 180. } else if yaw < -90. { yaw += 180. }
    Sample {
        tick: s.tick,
        state: s.skater.player_state.current() as u32,
        family: s.skater.grind.active_family().map_or(-1, |f| f as i32),
        deck: [position[0], position[1], position[2]],
        trucks: [front[0] - RAIL_X, rear[0] - RAIL_X],
        truck_height: [front[1] - RAIL_Y, rear[1] - RAIL_Y],
        truck_z: [front[2], rear[2]],
        yaw,
        along: -v.z,
        across: v.x,
        vy: v.y,
        targeting: s.skater.trajectory.selector.grind_locked_to_middle(),
        pelvis_x: s.root_position()[0] - RAIL_X,
    }
}

/// Approach direction for `angle` degrees off the rail, coming from the -x side toward -z.
fn direction(angle: f32) -> [f32; 3] {
    let a = angle.to_radians();
    [a.sin(), 0., -a.cos()]
}

/// Spawns `run_up` metres before `target` (x, z) along `direction(angle)`, rides at `speed`
/// and flicks an ollie `flick_at` metres before the target. Returns samples from the flick on.
fn ride(assets: &Assets, target: [f32; 2], angle: f32, speed: f32, flick_at: f32, ticks: usize) -> (Vec<Sample>, f32) {
    let d = direction(angle);
    let spawn = [target[0] - d[0] * RUN_UP, super::ground::FLOOR_HEIGHT, target[1] - d[2] * RUN_UP];
    // heading 0 faces +z; rotation_y(h) maps +z to (sin h, 0, cos h).
    let heading = d[0].atan2(d[2]);
    let physics = GamePhysics::load_course_at(&assets.root, Difficulty::Normal, spawn, heading).unwrap();
    let mut s = Session::new(assets, physics, |_| {});
    for _ in 0..SETTLE_TICKS {
        s.step(0, [0; 2]).unwrap();
    }
    crate::physics::impulse::queue(&mut s.physics, [d[0] * speed, 0., d[2] * speed]);
    let remaining = |s: &Session| {
        let p = super::solve::deck_frame(&s.physics.board)[3];
        (target[0] - p[0]) * d[0] + (target[1] - p[2]) * d[2]
    };
    let mut flick = None;
    let mut out = Vec::new();
    let mut speed_at_flick = 0.;
    for tick in 0..600 {
        if flick.is_none() && tick > 2 && remaining(&s) <= flick_at {
            flick = Some(tick);
            let v = s.physics.board.bodies()[BodyId::Deck.index()].rates.linear_velocity;
            speed_at_flick = (v.x * v.x + v.z * v.z).sqrt();
        }
        // Ollie as in the SK-031 test: right stick down 8 ticks, flick up 6.
        let right = match flick.map(|f| tick - f) {
            Some(0..=7) => [0, -32767],
            Some(8..=13) => [0, 32767],
            _ => [0, 0],
        };
        s.step_sticks(0, [0; 2], right).unwrap_or_else(|e| panic!("angle {angle} speed {speed}: {e}"));
        if flick.is_some() {
            out.push(sample(&s));
            if out.len() >= ticks { break; }
        }
    }
    (out, speed_at_flick)
}

/// Ollie on open floor (no rail near): distance from the flick to where the deck comes
/// down through rail height + DECK_OVER_RAIL (and the tick), the distance to where it first
/// rises through that height, and the deck apex over the floor.
fn calibrate(assets: &Assets, speed: f32) -> (f32, usize, f32, f32) {
    let target = [-25.0, 0.0];
    let (samples, _) = ride(assets, target, 0., speed, 3.0, 120);
    let start = samples[0].deck;
    let floor_deck = start[1];
    let apex = samples.iter().map(|s| s.deck[1]).fold(f32::MIN, f32::max);
    let peak = samples.iter().position(|s| s.deck[1] == apex).unwrap();
    let level = RAIL_Y + DECK_OVER_RAIL;
    let down = samples[peak..].iter().position(|s| s.deck[1] < level).map(|i| i + peak);
    let up = samples[..peak].iter().position(|s| s.deck[1] >= level);
    let (Some(down), Some(up)) = (down, up) else {
        return (f32::NAN, 0, f32::NAN, apex - floor_deck);
    };
    (start[2] - samples[down].deck[2], down, start[2] - samples[up].deck[2], apex - floor_deck)
}

#[derive(Default)]
struct Outcome {
    locked: Option<(usize, i32)>,
    targeting: bool,
    /// Worst truck lateral offset on the closest tick where both probes reached rail height
    /// over the rail span (the geometric condition for a 50-50 contact).
    miss: Option<f32>,
    pelvis_at_miss: Option<f32>,
    lateral_jump: [f32; 2],
    height_jump: f32,
    yaw_jump: [f32; 2],
    speed_loss: f32,
    across_before: f32,
}

fn measure(samples: &[Sample]) -> Outcome {
    let mut o = Outcome::default();
    o.targeting = samples.iter().any(|s| s.targeting);
    let lock = samples.iter().position(|s| (400..=405).contains(&s.state));
    // The lock tick's contact test sees the board as the previous tick left it.
    let window = lock.unwrap_or(samples.len());
    let reaches = |s: &Sample| (0..2).all(|t| s.truck_height[t] >= -0.001 && s.truck_height[t] <= 0.2
        && s.truck_z[t] >= RAIL_Z[0] && s.truck_z[t] <= RAIL_Z[1]);
    let best = samples[..window].iter().filter(|s| reaches(s))
        .min_by(|a, b| worst(a).total_cmp(&worst(b)));
    o.miss = best.map(worst);
    o.pelvis_at_miss = best.map(|s| s.pelvis_x);
    if let Some(l) = lock.filter(|&l| l > 0) {
        let (before, at) = (&samples[l - 1], &samples[l]);
        let after = &samples[(l + 10).min(samples.len() - 1)];
        let mid = |s: &Sample| (s.trucks[0] + s.trucks[1]) * 0.5;
        o.locked = Some((l, at.family));
        o.lateral_jump = [mid(at) - mid(before), mid(after) - mid(before)];
        o.height_jump = at.deck[1] - before.deck[1];
        o.yaw_jump = [at.yaw - before.yaw, after.yaw - before.yaw];
        let horizontal = |s: &Sample| (s.along * s.along + s.across * s.across).sqrt();
        let later = &samples[(l + 5).min(samples.len() - 1)];
        o.speed_loss = horizontal(before) - horizontal(later);
        o.across_before = before.across;
    }
    o
}

fn worst(s: &Sample) -> f32 {
    s.trucks[0].abs().max(s.trucks[1].abs())
}

fn write_csv(dir: &std::path::Path, name: &str, samples: &[Sample]) {
    use std::fmt::Write;
    let mut text = String::from("tick,state,family,deck_x,deck_y,deck_z,front_lat,rear_lat,front_h,rear_h,yaw,along,across,vy,targeting,pelvis_lat\n");
    for s in samples {
        writeln!(text, "{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.2},{:.3},{:.3},{:.3},{},{:.4}",
            s.tick, s.state, s.family, s.deck[0], s.deck[1], s.deck[2], s.trucks[0], s.trucks[1],
            s.truck_height[0], s.truck_height[1], s.yaw, s.along, s.across, s.vy, s.targeting as u8,
            s.pelvis_x).unwrap();
    }
    std::fs::write(dir.join(name), text).unwrap();
}

fn opt(v: Option<f32>) -> String {
    v.map_or("-".into(), |v| format!("{v:.3}"))
}

/// SK-062 sweep: lateral offset 0..1 m, angle 0/10/20/30, speed 3/5/7 m/s, Normal.
/// Prints one table row per run (`SK062 |...`). Set SK062_QUICK=1 for a 12-run subset.
#[test]
#[ignore = "requires private stock assets; SK-062 grind snap sweep (~10 min)"]
fn grind_snap_sweep() {
    let assets = Assets::load();
    let dir = std::env::var_os("SK062_OUT").map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("sk062"));
    std::fs::create_dir_all(&dir).unwrap();
    let quick = std::env::var_os("SK062_QUICK").is_some();
    let speeds: &[f32] = &[3., 5., 7.];
    let angles: &[f32] = if quick { &[0., 20.] } else { &[0., 10., 20., 30.] };
    let offsets: Vec<f32> = if quick { vec![0., 0.3] } else { (0..=10).map(|i| i as f32 * 0.1).collect() };
    eprintln!("SK062 csv dir {}", dir.display());
    eprintln!("SK062 | speed | angle | offset | v@flick | aim | locked | lock tick | miss | pelvis | dlat 1/10 | dy | dyaw 1/10 | v loss 5 | across | magnet");
    let mut locks = 0;
    for &speed in speeds {
        let (lead, down_tick, rise, apex) = calibrate(&assets, speed);
        // Come down over the rail as early as the near end allows (straight runs must clear it).
        let target_z = RAIL_Z[1] + END_CLEARANCE + rise - lead;
        eprintln!("SK062 calibrate speed {speed}: flick-to-rail-height {lead:.2} m, {down_tick} ticks, rise {rise:.2} m, deck apex {apex:.2} m, target z {target_z:.2}");
        assert!(lead.is_finite(), "ollie at {speed} m/s never reached {:.2} m over the rail top", DECK_OVER_RAIL);
        for &angle in angles {
            for &offset in &offsets {
                // Offset along -x: the approach side for angled runs.
                let target = [RAIL_X - offset, target_z];
                let (samples, v0) = ride(&assets, target, angle, speed, lead, 90);
                write_csv(&dir, &format!("s{speed}_a{angle}_o{offset:.1}.csv"), &samples);
                let o = measure(&samples);
                locks += o.locked.is_some() as u32;
                let aim = samples.get(down_tick).map(worst);
                eprintln!("SK062 | {speed} | {angle} | {offset:.1} | {v0:.2} | {} | {} | {} | {} | {} | {:.3}/{:.3} | {:.3} | {:.1}/{:.1} | {:.2} | {:.2} | {} |",
                    opt(aim),
                    o.locked.map_or("no".into(), |(_, f)| format!("fam{f}")),
                    o.locked.map_or("-".into(), |(t, _)| t.to_string()),
                    opt(o.miss), opt(o.pelvis_at_miss),
                    o.lateral_jump[0], o.lateral_jump[1], o.height_jump, o.yaw_jump[0], o.yaw_jump[1],
                    o.speed_loss, o.across_before, if o.targeting { "y" } else { "n" });
            }
        }
    }
    // Non-regression basic: a straight, centred approach must lock at some speed.
    assert!(locks > 0, "no run locked a grind");
}
