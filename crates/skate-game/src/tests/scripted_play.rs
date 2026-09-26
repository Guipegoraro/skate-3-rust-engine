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
    controls: PlayerControls,
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
            controls: PlayerControls::default(),
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
        self.input.sample_raw_for_test(XboxState { buttons, triggers: [0; 2], left, right: [0; 2] });
        let mut actions = self.input.player_actions();
        self.controls
            .update_for_physics(&mut actions, &self.physics, &self.skater, &self.camera)
            .map_err(|e| e.to_string())?;
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
