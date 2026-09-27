//! NPC pedestrians (SK-034, first version): a few puppets wander around the player on foot.
//! No physics and no interaction: each one walks to random nearby points, idles, and picks a
//! new point. Ground height and walls come from line probes against the collision world
//! (`world_probe`), the look from the stock skater or the retail roster, the pose from the
//! stock walk/stand cycles. Count: Game options "Pedestrians", or `SKATE3_PEDESTRIANS=<n>`.
use crate::physics::{GamePhysics, SkaterRuntime};
use crate::puppet::{Binding, LoadState, Look, Puppet, PuppetLoad, PuppetLoader, pose_locals};
use bevy::prelude::*;
use skate_core::animation::playback_tree::PoseCommand;
use std::f32::consts::{PI, TAU};

const WALK_CLIP: &str = "NB_WALK_FWD_CYC";
const STAND_CLIP: &str = "NB_STAND_0_CYC";
/// Used when the walk clip carries no loop translation.
const FALLBACK_WALK_SPEED: f32 = 1.3;
/// Pedestrians farther than this from the player are hidden and not posed.
const DRAW_DISTANCE: f32 = 70.;
/// The player moved this far from where the group spawned (teleport): respawn around them.
const RESPAWN_DISTANCE: f32 = 150.;

// ---------------------------------------------------------------------------------------------
// Wandering: pure decisions, tested below with a fake world.

/// What the wanderer can ask the world.
pub(crate) trait Terrain {
    /// Walkable ground height near `at` (from a little above to a little below it).
    fn ground(&self, at: Vec3) -> Option<f32>;
    /// A wall or steep slope between two points at knee/waist height.
    fn blocked(&self, from: Vec3, to: Vec3) -> bool;
}

/// Highest step up or down a pedestrian takes in one stride.
const MAX_STEP: f32 = 0.35;
/// How far ahead walls and drops are checked.
const LOOK_AHEAD: f32 = 0.7;
/// Turn speed, radians per second; pedestrians only walk when roughly facing the target.
const TURN_RATE: f32 = 2.5;
const FACING: f32 = 0.5;
/// Targets are picked within this distance of the spawn point.
const LEASH: f32 = 18.;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Idle(f32),
    Walk { target: Vec3, left: f32 },
}

/// One pedestrian's position, heading and plan. Forward is (sin yaw, 0, cos yaw).
#[derive(Clone, Debug)]
pub(crate) struct Wander {
    pub position: Vec3,
    pub yaw: f32,
    home: Vec3,
    mode: Mode,
    /// Set after bumping into something: the next target is picked away from this heading.
    avoid: Option<f32>,
    rng: Rng,
}
impl Wander {
    pub(crate) fn new(position: Vec3, seed: u64) -> Self {
        let mut rng = Rng(seed | 1);
        let yaw = rng.range(0., TAU);
        let idle = rng.range(0., 3.);
        Self { position, yaw, home: position, mode: Mode::Idle(idle), avoid: None, rng }
    }

    /// Walking toward a target and facing it with a clear path: the walk cycle should play.
    pub(crate) fn wants_to_walk(&self) -> bool {
        match self.mode {
            Mode::Walk { target, .. } => {
                let to = (target - self.position).with_y(0.);
                angle_difference(to.x.atan2(to.z), self.yaw).abs() <= FACING
            }
            Mode::Idle(_) => false,
        }
    }

    /// Advance by `dt` seconds at up to `speed` m/s; returns the speed actually walked
    /// (0 while idle or turning). The caller eases `speed` in with the walk animation.
    pub(crate) fn step(&mut self, dt: f32, speed: f32, terrain: &impl Terrain) -> f32 {
        match self.mode {
            Mode::Idle(left) => {
                self.mode = if left > dt { Mode::Idle(left - dt) } else { self.pick_target() };
                0.
            }
            Mode::Walk { target, left } => {
                let to = (target - self.position).with_y(0.);
                if to.length() < 0.4 || left <= dt {
                    self.mode = Mode::Idle(self.rng.range(1., 4.));
                    return 0.;
                }
                self.mode = Mode::Walk { target, left: left - dt };
                let wanted = to.x.atan2(to.z);
                let turn = angle_difference(wanted, self.yaw);
                self.yaw += turn.clamp(-TURN_RATE * dt, TURN_RATE * dt);
                if turn.abs() > FACING {
                    return 0.;
                }
                let forward = Vec3::new(self.yaw.sin(), 0., self.yaw.cos());
                let next = self.position + forward * (speed * dt).min(to.length());
                if self.clear(forward, terrain) {
                    if let Some(y) = terrain.ground(next).filter(|y| (y - self.position.y).abs() <= MAX_STEP) {
                        self.position = next.with_y(y);
                        return speed;
                    }
                }
                self.avoid = Some(self.yaw);
                self.mode = Mode::Idle(self.rng.range(0.5, 1.5));
                0.
            }
        }
    }

    /// No wall ahead and ground (no big drop or step) a stride ahead.
    fn clear(&self, forward: Vec3, terrain: &impl Terrain) -> bool {
        let ahead = self.position + forward * LOOK_AHEAD;
        let waist = Vec3::Y * 0.6;
        let knee = Vec3::Y * (MAX_STEP + 0.05);
        !terrain.blocked(self.position + waist, ahead + waist)
            && !terrain.blocked(self.position + knee, ahead + knee)
            && terrain
                .ground(ahead)
                .is_some_and(|y| (y - self.position.y).abs() <= MAX_STEP * 1.5)
    }

    fn pick_target(&mut self) -> Mode {
        let home = (self.home - self.position).with_y(0.);
        let heading = if let Some(avoid) = self.avoid.take() {
            avoid + PI + self.rng.range(-1., 1.)
        } else if home.length() > LEASH {
            home.x.atan2(home.z) + self.rng.range(-0.6, 0.6)
        } else {
            self.rng.range(0., TAU)
        };
        let distance = self.rng.range(3., 10.);
        Mode::Walk {
            target: self.position + Vec3::new(heading.sin(), 0., heading.cos()) * distance,
            left: distance / 0.5 + 4.,
        }
    }
}

/// Signed smallest angle from `b` to `a`, in -PI..PI.
fn angle_difference(a: f32, b: f32) -> f32 {
    (a - b + PI).rem_euclid(TAU) - PI
}

/// Small xorshift generator; the wander logic needs no quality randomness.
#[derive(Clone, Debug)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (self.next() >> 40) as f32 / (1u64 << 24) as f32 * (high - low)
    }
}

// ---------------------------------------------------------------------------------------------
// The collision world as terrain.

struct World<'a>(&'a skate_core::physics::board_world::BoardWorld);
impl Terrain for World<'_> {
    fn ground(&self, at: Vec3) -> Option<f32> {
        crate::world_probe::ground_below(self.0, at + Vec3::Y * (MAX_STEP * 1.5 + 0.1), MAX_STEP * 3. + 0.4, 0.7)
            .map(|p| p.y)
    }
    fn blocked(&self, from: Vec3, to: Vec3) -> bool {
        crate::world_probe::wall_between(self.0, from, to, 0.7)
    }
}

// ---------------------------------------------------------------------------------------------
// Animation: the stock stand and walk cycles, blended by walking weight.

struct Cycles {
    walk: f32,
    stand: Option<f32>,
    /// Walk speed that matches the clip's stride, metres per second.
    speed: f32,
    /// Model-space heading of the clip's forward motion.
    model_yaw: f32,
}
impl Cycles {
    fn load(skater: &SkaterRuntime) -> Result<Self, String> {
        let frames = &skater.animation.evaluator.frames;
        let length = |name: &str| -> Result<f32, String> {
            let clip = frames.clip(name)?;
            Ok((clip.frames.len().max(2) - 1) as f32 / f32::from_bits(clip.fps_bits))
        };
        let walk = length(WALK_CLIP)?;
        let travel = frames.clip(WALK_CLIP)?.loop_translation_bits.map(f32::from_bits);
        let stride = Vec2::new(travel[0], travel[2]);
        let (speed, model_yaw) = if stride.length() > 0.1 && walk > 0. {
            (stride.length() / walk, stride.x.atan2(stride.y))
        } else {
            (FALLBACK_WALK_SPEED, 0.)
        };
        Ok(Self { walk, stand: length(STAND_CLIP).ok(), speed, model_yaw })
    }

    /// Model-space bone matrices for a stand/walk blend (`weight` 1 = walking).
    fn pose(&self, skater: &SkaterRuntime, stand_time: f32, walk_time: f32, weight: f32) -> Option<Vec<Mat4>> {
        let clip = |name: &str, time: f32| PoseCommand::Clip {
            name: name.into(),
            previous_time: time,
            time,
            loops: 0,
        };
        let walk = clip(WALK_CLIP, walk_time.rem_euclid(self.walk));
        let mut commands = match self.stand {
            Some(length) if weight < 0.999 => {
                let stand = clip(STAND_CLIP, stand_time.rem_euclid(length));
                if weight > 0.001 {
                    vec![stand, walk, PoseCommand::Blend { weight }]
                } else {
                    vec![stand]
                }
            }
            _ => vec![walk],
        };
        commands.push(PoseCommand::Pose { name: "RIG_TPOSE".into() });
        commands.push(PoseCommand::Add { motion_is_a: true });
        let evaluator = &skater.animation.evaluator;
        let pose = evaluator.evaluate(&commands).ok()?;
        let globals = evaluator.hierarchy(&pose).ok()?;
        Some(globals.into_iter().map(crate::animation::native_matrix).collect())
    }
}

// ---------------------------------------------------------------------------------------------
// ECS side.

struct Pedestrian {
    root: Entity,
    load: Option<PuppetLoad>,
    bindings: Vec<Binding>,
    wander: Wander,
    stand_time: f32,
    walk_time: f32,
    weight: f32,
}

#[derive(Resource, Default)]
struct Pedestrians {
    actors: Vec<Pedestrian>,
    cycles: Option<Result<Cycles, String>>,
    /// Where the current group spawned.
    center: Option<Vec3>,
    player: Option<Vec3>,
    seed: u64,
}

pub(crate) struct PedestriansPlugin;
impl Plugin for PedestriansPlugin {
    fn build(&self, app: &mut App) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        app.insert_resource(Pedestrians { seed, ..default() }).add_systems(
            Update,
            (manage, walk).chain().after(crate::modding::vehicles::present),
        );
    }
}

fn wanted_count(options: &crate::game_options::GameOptions) -> usize {
    let count = std::env::var("SKATE3_PEDESTRIANS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(options.pedestrians);
    count.min(crate::game_options::MAX_PEDESTRIANS) as usize
}

/// Spawn, despawn and finish loading pedestrians.
fn manage(
    mut loader: PuppetLoader,
    mut state: ResMut<Pedestrians>,
    mut changed: MessageReader<crate::map_transition::WorldChanged>,
    options: Res<crate::game_options::GameOptions>,
    animation: Res<crate::animation::AnimationStatus>,
    physics: Res<GamePhysics>,
    skater: Res<SkaterRuntime>,
    models: Res<crate::custom_models::CustomModels>,
    rig: Res<crate::puppet::PuppetRig>,
    player: Query<&Transform, With<crate::world::PlayerRoot>>,
) {
    let state = &mut *state;
    let player = player.iter().next().map(|t| t.translation).filter(|_| animation.ready);
    state.player = player;
    let moved_away = state
        .center
        .zip(player)
        .is_some_and(|(c, p)| c.distance(p) > RESPAWN_DISTANCE);
    let wanted = wanted_count(&options);
    if changed.read().count() > 0 || moved_away || wanted == 0 {
        for p in state.actors.drain(..) {
            loader.commands().entity(p.root).despawn();
        }
        state.center = None;
    }
    while state.actors.len() > wanted {
        let p = state.actors.pop().unwrap();
        loader.commands().entity(p.root).despawn();
    }
    let Some(player) = player else { return };
    if state.actors.len() < wanted {
        let cycles = state.cycles.get_or_insert_with(|| {
            let cycles = Cycles::load(&skater);
            match &cycles {
                Ok(c) => info!("PEDESTRIANS walk speed {:.2} m/s, cycle {:.2} s, model yaw {:.2}, stand {:?}",
                    c.speed, c.walk, c.model_yaw, c.stand),
                Err(e) => warn!("Pedestrians disabled: {e}"),
            }
            cycles
        });
        if cycles.is_err() {
            return;
        }
        let roster = models.native_keys();
        let world = World(physics.world());
        // A few tries per frame; places without ground (or behind a wall) are skipped.
        for _ in 0..8 {
            if state.actors.len() >= wanted {
                break;
            }
            state.seed = state.seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let mut rng = Rng(state.seed | 1);
            let angle = rng.range(0., TAU);
            let spot = player + Vec3::new(angle.sin(), 0., angle.cos()) * rng.range(4., 14.);
            let Some(ground) = crate::world_probe::ground_below(physics.world(), spot + Vec3::Y * 3., 6., 0.7)
            else {
                continue;
            };
            if world.blocked(player + Vec3::Y * 0.8, ground + Vec3::Y * 0.8) {
                continue;
            }
            // Mostly roster characters for variety; the stock skater when none are installed.
            let pick = rng.next() as usize % (roster.len() + 1);
            let look = roster.get(pick).cloned().map_or(Look::Stock, Look::Native);
            let root = loader
                .commands()
                .spawn((
                    Puppet,
                    Transform::from_translation(ground),
                    Visibility::Inherited,
                    Name::new("Pedestrian"),
                ))
                .id();
            let load = loader.spawn(root, look);
            state.actors.push(Pedestrian {
                root,
                load,
                bindings: vec![],
                wander: Wander::new(ground, rng.next()),
                stand_time: rng.range(0., 10.),
                walk_time: rng.range(0., 10.),
                weight: 0.,
            });
            state.center.get_or_insert(player);
        }
    }
    for p in &mut state.actors {
        let Some(load) = p.load.as_ref() else { continue };
        match loader.poll(load, true) {
            LoadState::Waiting => {}
            LoadState::Failed(e) => {
                warn!("Pedestrian model failed ({e})");
                p.load = None;
            }
            LoadState::Ready(bindings) => {
                // Shown in the rest pose; `walk` poses it from the next frame on.
                loader.show(load, &bindings, &rig.globals([]));
                p.bindings = bindings;
                p.load = None;
            }
        }
    }
}

/// Move, turn and pose every loaded pedestrian near the player.
fn walk(
    time: Res<Time>,
    mut state: ResMut<Pedestrians>,
    physics: Res<GamePhysics>,
    skater: Res<SkaterRuntime>,
    rig: Res<crate::puppet::PuppetRig>,
    mut nodes: Query<&mut Transform>,
    mut visibility: Query<&mut Visibility>,
) {
    let state = &mut *state;
    let (Some(Ok(cycles)), Some(player)) = (&state.cycles, state.player) else { return };
    let dt = time.delta_secs().min(0.1);
    let world = World(physics.world());
    for p in &mut state.actors {
        if p.bindings.is_empty() {
            continue;
        }
        let near = p.wander.position.distance(player) < DRAW_DISTANCE;
        if let Ok(mut visibility) = visibility.get_mut(p.root) {
            visibility.set_if_neq(if near { Visibility::Inherited } else { Visibility::Hidden });
        }
        if !near {
            continue;
        }
        // Ground speed follows the stand/walk blend, so the feet never slide: half-blended
        // strides cover about half the distance while the cycle keeps its own cadence.
        p.wander.step(dt, cycles.speed * p.weight, &world);
        let target = if p.wander.wants_to_walk() { 1. } else { 0. };
        p.weight += (target - p.weight).clamp(-dt * 3., dt * 3.);
        p.walk_time += dt;
        p.stand_time += dt;
        let Some(globals) = cycles.pose(&skater, p.stand_time, p.walk_time, p.weight) else { continue };
        if let Ok(mut t) = nodes.get_mut(p.root) {
            t.translation = p.wander.position;
            t.rotation = Quat::from_rotation_y(p.wander.yaw - cycles.model_yaw);
        }
        for (&(entity, bone, _), local) in p.bindings.iter().zip(pose_locals(&p.bindings, &globals)) {
            if let Ok(mut t) = nodes.get_mut(entity) {
                *t = local;
                // Pedestrians carry no board.
                if rig.board == Some(bone) {
                    t.scale = Vec3::splat(0.001);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat ground at y=0 for x < `edge`, a wall at x = `wall`.
    struct Fake {
        wall: f32,
        edge: f32,
    }
    impl Terrain for Fake {
        fn ground(&self, at: Vec3) -> Option<f32> {
            (at.x < self.edge).then_some(0.)
        }
        fn blocked(&self, from: Vec3, to: Vec3) -> bool {
            (from.x - self.wall).signum() != (to.x - self.wall).signum()
        }
    }

    fn run(w: &mut Wander, terrain: &Fake, seconds: f32) -> (f32, f32) {
        let (mut walked, mut max_x) = (0., f32::MIN);
        for _ in 0..(seconds * 60.) as usize {
            walked += w.step(1. / 60., 1.3, terrain) / 60.;
            max_x = max_x.max(w.position.x);
        }
        (walked, max_x)
    }

    #[test]
    fn wanders_on_open_ground_and_stays_on_its_leash() {
        let open = Fake { wall: 1e6, edge: 1e6 };
        for seed in 1..20 {
            let mut w = Wander::new(Vec3::ZERO, seed);
            let (walked, _) = run(&mut w, &open, 120.);
            assert!(walked > 20., "seed {seed} walked only {walked} m");
            assert!(w.position.length() < LEASH + 12., "seed {seed} strayed to {}", w.position);
            assert_eq!(w.position.y, 0.);
        }
    }

    #[test]
    fn never_crosses_a_wall_or_walks_off_an_edge() {
        let walled = Fake { wall: 2., edge: 1e6 };
        let cliff = Fake { wall: 1e6, edge: 2. };
        for seed in 1..20 {
            let (walked, max_x) = run(&mut Wander::new(Vec3::ZERO, seed), &walled, 120.);
            assert!(max_x < 2., "seed {seed} went through the wall to x={max_x}");
            assert!(walked > 10.);
            let (_, max_x) = run(&mut Wander::new(Vec3::ZERO, seed), &cliff, 120.);
            assert!(max_x < 2., "seed {seed} walked off the edge to x={max_x}");
        }
    }

    #[test]
    fn turns_before_walking() {
        let open = Fake { wall: 1e6, edge: 1e6 };
        let mut w = Wander::new(Vec3::ZERO, 7);
        w.mode = Mode::Walk { target: Vec3::new(0., 0., -5.), left: 20. };
        w.yaw = 0.;
        assert_eq!(w.step(1. / 60., 1.3, &open), 0., "facing away: turn first");
        run(&mut w, &open, 3.);
        assert!(w.position.z < -1.);
    }

    #[test]
    fn angle_difference_wraps() {
        assert!((angle_difference(0.1, TAU - 0.1) - 0.2).abs() < 1e-5);
        assert!((angle_difference(-3., 3.) - (TAU - 6.)).abs() < 1e-5);
    }
}
