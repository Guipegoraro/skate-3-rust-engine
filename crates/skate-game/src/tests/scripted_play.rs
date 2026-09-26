//! Scripted play harness: load stock assets once, then drive the full fixed tick with a
//! raw controller script and observe the physics. Tests here need SKATE3_ASSET_ROOT (an
//! absolute path) and run with `--ignored`. See docs/testing-notes.md.
use super::*;
use skate_core::input::xbox::XboxState;

pub(crate) const A: u16 = 0x1000;
pub(crate) const X: u16 = 0x4000;
pub(crate) const Y: u16 = 0x8000;

pub(crate) struct Assets {
    pub root: std::path::PathBuf,
    pub graphs: crate::graph_runtime::StockGraphs,
}
impl Assets {
    pub fn load() -> Self {
        let root = std::path::PathBuf::from(
            std::env::var_os("SKATE3_ASSET_ROOT").expect("set SKATE3_ASSET_ROOT (absolute path)"),
        );
        let assets = skate_data::GameAssets::load(&root).unwrap();
        let graphs = crate::graph_runtime::StockGraphs::load(&root, &assets).unwrap();
        Self { root, graphs }
    }
}

/// One skater on one world, stepped one 60 Hz tick per `step`.
pub(crate) struct Session<'a> {
    pub physics: GamePhysics,
    pub skater: SkaterRuntime,
    pub tick: usize,
    pub controls: PlayerControls,
    input: crate::input::ControllerInput,
    camera: crate::camera::CameraRuntime,
    graphs: &'a crate::graph_runtime::StockGraphs,
}
impl<'a> Session<'a> {
    /// `configure` runs before the skater loads (trainer, gravity, ...).
    pub fn new(assets: &'a Assets, physics: GamePhysics, configure: impl FnOnce(&mut GamePhysics)) -> Self {
        let mut physics = physics;
        configure(&mut physics);
        let skater = SkaterRuntime::load(&assets.root, &assets.graphs, &physics, "normal").unwrap();
        Self {
            physics,
            skater,
            tick: 0,
            controls: PlayerControls::load(&assets.root).unwrap(),
            input: Default::default(),
            camera: crate::camera::CameraRuntime::load(&assets.root).unwrap(),
            graphs: &assets.graphs,
        }
    }
    /// Test world (flat ground); pass a map to start on a real level.
    pub fn flat(assets: &'a Assets, configure: impl FnOnce(&mut GamePhysics)) -> Self {
        Self::new(assets, GamePhysics::load(&assets.root).unwrap(), configure)
    }
    pub fn step(&mut self, buttons: u16, left: [i16; 2]) -> Result<(), String> {
        self.step_sticks(buttons, left, [0; 2])
    }
    /// Sticks in raw XInput units (y up).
    pub fn step_sticks(&mut self, buttons: u16, left: [i16; 2], right: [i16; 2]) -> Result<(), String> {
        self.input.sample_raw_for_test(XboxState { buttons, triggers: [0; 2], left, right });
        let mut actions = self.input.player_actions();
        self.controls
            .update_for_physics(&mut actions, &self.physics, &self.skater, &self.camera)
            .map_err(|e| e.to_string())?;
        // Same order as the game's controls::sample system.
        self.controls.publish_gestures(
            self.physics.animation_profile.physics_mode,
            self.skater.player_input.physical.state.state_16,
        );
        frame::advance(
            &mut self.physics,
            &mut self.skater,
            &mut self.controls,
            self.graphs,
            &mut actions,
            true,
            &mut self.camera,
        )
        .map_err(|e| format!("tick{}: {e}", self.tick))?;
        self.tick += 1;
        Ok(())
    }
    /// Root body (pelvis) position.
    pub fn root_position(&self) -> [f32; 3] {
        let p = self.skater.skeleton.bodies()[0].rates.position;
        [p.x, p.y, p.z]
    }
}

/// On foot on flat ground: tap X and return how high the pelvis rises.
fn offboard_jump_height(assets: &Assets, gravity: f32) -> f32 {
    let mut s = Session::flat(assets, |p| p.gravity = gravity);
    // Y steps off the board; settle on foot before jumping.
    for tick in 0..120 {
        s.step(if tick == 20 { Y } else { 0 }, [0; 2]).unwrap();
    }
    let base = s.root_position()[1];
    let mut peak = base;
    for tick in 0..240 {
        s.step(if tick < 6 { X } else { 0 }, [0; 2]).unwrap();
        peak = peak.max(s.root_position()[1]);
    }
    peak - base
}

/// SK-004: sdk.physics.gravity lowers gravity, so the same jump goes higher.
#[test]
#[ignore = "requires private stock assets; SK-004 mod gravity"]
fn mod_gravity_scales_offboard_jump_height() {
    let assets = Assets::load();
    let stock = offboard_jump_height(&assets, 1.0);
    let low = offboard_jump_height(&assets, 0.5);
    eprintln!("SK004 jump height stock={stock} gravity0.5={low}");
    assert!(stock > 0.1, "no jump happened: {stock}");
    assert!(low > stock * 1.3, "half gravity did not raise the jump: stock={stock} low={low}");
}

/// SK-005: sdk.player.impulse adds velocity while riding, and nothing on foot.
#[test]
#[ignore = "requires private stock assets; SK-005 mod impulse"]
fn mod_impulse_changes_riding_velocity_only() {
    let assets = Assets::load();
    let deck_speed = |s: &Session| {
        let v = s.physics.board.bodies()[0].rates.linear_velocity;
        (v.x * v.x + v.z * v.z).sqrt()
    };
    let mut s = Session::flat(&assets, |_| {});
    for _ in 0..60 {
        s.step(0, [0; 2]).unwrap();
    }
    let before = deck_speed(&s);
    crate::physics::impulse::queue(&mut s.physics, [8., 0., 0.]);
    s.step(0, [0; 2]).unwrap();
    let after = deck_speed(&s);
    eprintln!("SK005 riding speed before={before} after={after}");
    assert!(after > before + 5., "riding impulse had no effect: {before} -> {after}");
    // On foot the state owns velocity: the request is dropped, not left pending.
    for tick in 0..120 {
        s.step(if tick == 0 { Y } else { 0 }, [0; 2]).unwrap();
    }
    crate::physics::impulse::queue(&mut s.physics, [8., 0., 0.]);
    s.step(0, [0; 2]).unwrap();
    assert!(s.physics.pending_impulse.is_none());
}

/// Right-stick XInput value from pattern space (x right, y down, -1..1).
pub(crate) fn stick_from_pattern([x, y]: [f32; 2]) -> [i16; 2] {
    [(x.clamp(-1., 1.) * 32767.) as i16, (-y.clamp(-1., 1.) * 32767.) as i16]
}

/// SK-027: flicking the stock Kickflip pattern records its name and ideal geometry for the HUD.
#[test]
#[ignore = "requires private stock assets; SK-027 recognized gesture"]
fn recognized_kickflip_carries_pattern_geometry() {
    let assets = Assets::load();
    let patterns = skate_data::gesture_patterns::load(
        &assets.root.join("private/stock/data/joystick/skater.pat")).unwrap();
    let kickflip = patterns.iter().find(|p| p.name == "Kickflip").expect("Kickflip pattern");
    let mut s = Session::flat(&assets, |_| {});
    for _ in 0..60 {
        s.step(0, [0; 2]).unwrap();
    }
    // Walk the key points, a few ticks on each, then release.
    let mut path = vec![[0., 0.]];
    path.extend(kickflip.points.iter().copied());
    for window in path.windows(2) {
        for k in 1..=4 {
            let t = k as f32 / 4.;
            let p = [window[0][0] + (window[1][0] - window[0][0]) * t, window[0][1] + (window[1][1] - window[0][1]) * t];
            s.step_sticks(0, [0; 2], stick_from_pattern(p)).unwrap();
        }
    }
    for _ in 0..10 {
        s.step(0, [0; 2]).unwrap();
    }
    let recognized = s.controls.recognized_gesture().expect("gesture input loaded");
    eprintln!("SK027 recognized {} x{} points {:?}", recognized.name, recognized.count, recognized.points);
    assert!(recognized.count > 0, "no right-stick trick recognized for {:?}", kickflip.points);
    assert_eq!(recognized.name, "Kickflip");
    assert_eq!(recognized.points, kickflip.points);
}

/// Crash report 26/09: respawn after a bail at this University checkpoint went
/// PhysicsGround -> PhysicsAir -> PhysicsGround and the board torque became NaN.
#[test]
#[ignore = "requires private stock assets and University.skate; bail checkpoint NaN"]
fn university_bail_checkpoint_respawn_stays_finite() {
    let assets = Assets::load();
    let path = std::env::var_os("SK_MAP").expect("set SK_MAP to University.skate");
    let mut map = skate_data::skate_map::SkateMap::load(std::path::Path::new(&path)).unwrap();
    for i in 0..16 {
        map.spawn = [375.8947, 125.11455, -713.6659];
        map.heading = i as f32 * std::f32::consts::TAU / 16.;
        let physics = GamePhysics::load_with_map(&assets.root, Some(&map)).unwrap();
        let mut s = Session::new(&assets, physics, |_| {});
        for _ in 0..240 {
            s.step(0, [0; 2]).unwrap_or_else(|e| panic!("heading {}: {e}", map.heading));
        }
        eprintln!("CHECKPOINT heading {} ok state {:?}", map.heading, s.skater.player_state.current());
    }
}
