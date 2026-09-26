//! Physics crash recovery (SK-033). When the physics rejects a non-finite state, the game no
//! longer closes: the report is saved to `logs/crash-<time>.txt`, the current map is reloaded
//! through the normal map transition (fresh physics, skater and controls), and the skater is
//! put back where it was riding a couple of seconds earlier. `SKATE_FAIL_FAST=1` (and
//! automated verification runs) keep the old behaviour of exiting with an error.
use bevy::prelude::*;
use std::collections::VecDeque;
use std::path::PathBuf;

/// Restore to a spot at least this old, so the skater is not put back into the moment that failed.
const RESTORE_AGE: f64 = 2.0;
const HISTORY_SECONDS: f64 = 10.0;
const SAMPLE_SECONDS: f64 = 0.5;

pub(crate) struct CrashRecoveryPlugin;
impl Plugin for CrashRecoveryPlugin {
    fn build(&self, app: &mut App) {
        // Verification captures must still fail loudly.
        if app.world().resource::<crate::config::Config>().verification_capture.is_some() {
            return;
        }
        app.init_resource::<CrashRecovery>()
            .add_systems(Update, (record, recover, restore).chain());
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spot {
    pub position: [f32; 3],
    pub heading: f32,
    pub on_board: bool,
}

#[derive(Resource, Default)]
pub(crate) struct CrashRecovery {
    history: VecDeque<(f64, Spot)>,
    failure: Option<String>,
    pending: Option<(Spot, f64)>,
    notice: Option<String>,
    pub recovered: u32,
}

impl CrashRecovery {
    pub fn physics_failed(&mut self, report: String) {
        if self.failure.is_none() {
            self.failure = Some(report);
        }
    }

    fn push(&mut self, now: f64, spot: Spot) {
        if self.history.back().is_some_and(|(t, _)| now - t < SAMPLE_SECONDS) {
            return;
        }
        self.history.push_back((now, spot));
        while self.history.front().is_some_and(|(t, _)| now - t > HISTORY_SECONDS) {
            self.history.pop_front();
        }
    }

    /// Newest spot at least RESTORE_AGE old, else the oldest one known.
    fn restore_spot(&self, now: f64) -> Option<Spot> {
        self.history.iter().rev().find(|(t, _)| now - t >= RESTORE_AGE)
            .or_else(|| self.history.front())
            .map(|(_, spot)| *spot)
    }
}

pub(crate) fn fail_fast() -> bool {
    std::env::var("SKATE_FAIL_FAST").is_ok_and(|v| v != "0")
}

fn spot(physics: &crate::physics::GamePhysics, skater: &crate::physics::SkaterRuntime) -> Option<Spot> {
    let root = skater.animated_skeleton.roots.animation_to_world;
    let position: [f32; 3] = root[3][..3].try_into().ok()?;
    let heading = root[2][0].atan2(root[2][2]);
    let physical = &skater.player_input.physical.state;
    // Never while bailing, resetting or teleporting.
    let settled = !physics.board_wiping_out && physical.flag_69 == 0 && physical.state_16 != 702;
    (settled && position.iter().all(|v| v.is_finite()) && heading.is_finite())
        .then_some(Spot { position, heading, on_board: physical.category_12 != 500 })
}

fn record(
    time: Res<Time<Real>>,
    physics: Option<ResMut<crate::physics::GamePhysics>>,
    skater: Option<Res<crate::physics::SkaterRuntime>>,
    mut recovery: ResMut<CrashRecovery>,
    mut forced: Local<bool>,
) {
    let (Some(mut physics), Some(skater)) = (physics, skater) else { return };
    if physics.failed {
        return;
    }
    // Test hook: SKATE_FORCE_PHYSICS_FAILURE=<seconds> fakes one failure after that long.
    let force_at = std::env::var("SKATE_FORCE_PHYSICS_FAILURE").ok().and_then(|v| v.parse::<f64>().ok());
    if !*forced && force_at.is_some_and(|at| time.elapsed_secs_f64() >= at) {
        *forced = true;
        physics.failed = true;
        recovery.physics_failed("Forced physics failure (SKATE_FORCE_PHYSICS_FAILURE)".into());
        return;
    }
    if let Some(spot) = spot(&physics, &skater) {
        recovery.push(time.elapsed_secs_f64(), spot);
    }
}

fn write_report(report: &str) -> Option<PathBuf> {
    let directory = PathBuf::from("logs");
    std::fs::create_dir_all(&directory).ok()?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    let path = directory.join(format!("crash-{stamp}.txt"));
    let text = format!("Skate 3 Rust Engine physics failure (recovered, SK-033)\nrevision: {}\n\n{report}\n",
        option_env!("SKATE_BUILD_REVISION").unwrap_or("unknown"));
    std::fs::write(&path, text).ok()?;
    Some(path)
}

/// Exclusive: rebuild the world through the map transition once a failure is reported.
fn recover(world: &mut World) {
    let Some(report) = world.resource_mut::<CrashRecovery>().failure.take() else { return };
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let saved = write_report(&report);
    let current = world.resource::<crate::map_transition::CurrentMap>();
    let entry = crate::map_library::Entry { label: current.name.clone(), path: current.path.clone() };
    {
        let mut recovery = world.resource_mut::<CrashRecovery>();
        recovery.pending = recovery.restore_spot(now).map(|spot| (spot, now));
        recovery.recovered += 1;
        recovery.history.clear();
    }
    warn!("CRASH_RECOVERY physics failure; report={saved:?}; reloading {:?}", entry.label);
    world.resource_mut::<crate::map_transition::MapTransition>().request(entry);
    let mut recovery = world.resource_mut::<CrashRecovery>();
    recovery.notice = Some(match &saved {
        Some(path) => format!("Physics error recovered. Report saved to {}", path.display()),
        None => "Physics error recovered.".to_string(),
    });
}

/// After the reload, put the skater back where it was (retrying while the new world settles).
fn restore(world: &mut World) {
    let Some((spot, since)) = world.resource::<CrashRecovery>().pending else { return };
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    if now - since > 20.0 {
        world.resource_mut::<CrashRecovery>().pending = None;
        return;
    }
    if world.resource::<crate::map_transition::MapTransition>().busy() {
        return;
    }
    let mut skater = world.resource_mut::<crate::physics::SkaterRuntime>();
    let physical = &skater.player_input.physical.state;
    if physical.flag_69 != 0 || physical.state_16 == 702 || skater.player_input.pending_teleport().is_some() {
        return;
    }
    let (sin, cos) = spot.heading.sin_cos();
    let transform = [
        [cos, 0., -sin, 0.],
        [0., 1., 0., 0.],
        [sin, 0., cos, 0.],
        [spot.position[0], spot.position[1], spot.position[2], 0.],
    ];
    if skater.player_input.request_teleport(transform).is_ok() {
        skater.teleport_state.request_manual(transform, spot.on_board);
        info!("CRASH_RECOVERY restored to {:?}", spot.position);
        let notice = world.resource_mut::<CrashRecovery>().notice.take();
        world.resource_mut::<CrashRecovery>().pending = None;
        // Shown in the pause menu status line (the reload's own notice replaced it).
        if let Some(notice) = notice {
            if let Some(mut menu) = world.get_resource_mut::<crate::graphics_menu::Menu>() { menu.transition_finished(notice, true); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> Spot {
        Spot { position: [x, 0., 0.], heading: 0., on_board: true }
    }

    #[test]
    fn restores_a_spot_older_than_the_failure_window() {
        let mut r = CrashRecovery::default();
        for i in 0..40 {
            r.push(i as f64 * 0.25, at(i as f32));
        }
        // Samples every 0.5 s up to t=9.5; at t=10 the newest spot >= 2 s old is t=8.
        assert_eq!(r.restore_spot(10.0), Some(at(32.)));
        // Only recent samples: fall back to the oldest.
        let mut fresh = CrashRecovery::default();
        fresh.push(0.0, at(1.));
        assert_eq!(fresh.restore_spot(0.5), Some(at(1.)));
        assert_eq!(CrashRecovery::default().restore_spot(1.0), None);
    }

    #[test]
    fn history_keeps_ten_seconds() {
        let mut r = CrashRecovery::default();
        for i in 0..100 {
            r.push(i as f64, at(i as f32));
        }
        assert!(r.history.front().unwrap().0 >= 89.0);
    }
}
