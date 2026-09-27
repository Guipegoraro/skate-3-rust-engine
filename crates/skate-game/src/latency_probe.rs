//! `SKATE_LATENCY_PROBE=1` (SK-060): logs, for each controller button press, the frame the pad
//! poll saw it, the 60 Hz physics tick that consumed it, the first frame that drew that tick
//! (partly, with smoothing On) and the frame that drew it fully. Times are from the pad poll.
//! Bevy's pipelined renderer draws a frame while the next one updates, and the swap chain adds
//! its own queue, so the screen shows a frame about one frame time after "full"; the probe
//! does not see that part (needs a high-speed camera). Off unless the variable is set.
use bevy::prelude::*;
use std::time::Instant;

pub(crate) struct LatencyProbePlugin;
impl Plugin for LatencyProbePlugin {
    fn build(&self, app: &mut App) {
        if std::env::var_os("SKATE_LATENCY_PROBE").is_none_or(|v| v.is_empty() || v == "0") {
            return;
        }
        app.init_resource::<Probe>()
            .add_systems(PreUpdate, poll.after(crate::input::poll_controllers))
            .add_systems(FixedUpdate, tick.after(crate::presentation::capture))
            .add_systems(PostUpdate, present);
        info!("SKATE_LATENCY_PROBE: logging button press -> physics tick -> drawn frame");
    }
}

#[derive(Clone, Copy)]
struct Press {
    buttons: u16,
    frame: u64,
    polled: Instant,
    tick: Option<(u64, Instant)>,
    visible: Option<(u64, Instant, f32)>,
}

/// Press bookkeeping, kept free of Bevy so it can be unit-tested.
#[derive(Resource, Default)]
pub(crate) struct Probe {
    frame: u64,
    tick: u64,
    previous_buttons: u16,
    pending: Vec<Press>,
    generation: u64,
    /// Ticks that produced the previous and newest presentation snapshots.
    snapshot_ticks: (u64, u64),
    /// Poll-to-full milliseconds of every finished press, for the running mean.
    total_ms: f64,
    count: u32,
}

fn ms(from: Instant, to: Instant) -> f64 {
    to.saturating_duration_since(from).as_secs_f64() * 1000.
}

impl Probe {
    /// Once per frame after the pad poll; new button bits start a press.
    fn poll(&mut self, now: Instant, buttons: u16) {
        self.frame += 1;
        let pressed = buttons & !self.previous_buttons;
        self.previous_buttons = buttons;
        if pressed != 0 {
            self.pending.push(Press { buttons: pressed, frame: self.frame, polled: now, tick: None, visible: None });
        }
        // A press made while the menu is open never reaches physics; drop it after 2 s.
        self.pending.retain(|p| now.saturating_duration_since(p.polled).as_secs_f32() < 2.);
    }
    /// Once per fixed tick after physics and the presentation capture.
    fn tick(&mut self, now: Instant, generation: u64) {
        self.tick += 1;
        for press in self.pending.iter_mut().filter(|p| p.tick.is_none()) {
            press.tick = Some((self.tick, now));
        }
        if generation != self.generation {
            self.generation = generation;
            self.snapshot_ticks = (self.snapshot_ticks.1, self.tick);
        }
    }
    /// Once per frame after presentation; `alpha` is the weight of the newest snapshot
    /// (1 with smoothing Off). Returns a log line per press that is now fully drawn.
    fn present(&mut self, now: Instant, alpha: f32) -> Vec<String> {
        let (previous, newest) = self.snapshot_ticks;
        let mut lines = Vec::new();
        let frame = self.frame;
        for press in &mut self.pending {
            let Some((tick, _)) = press.tick else { continue };
            let weight = if newest < tick { 0. } else if previous >= tick { 1. } else { alpha };
            if weight > 0. && press.visible.is_none() {
                press.visible = Some((frame, now, weight));
            }
            if weight >= 0.999 {
                let (tick, ticked) = press.tick.unwrap();
                let (seen, seen_at, seen_weight) = press.visible.unwrap();
                let full = ms(press.polled, now);
                self.total_ms += full;
                self.count += 1;
                lines.push(format!(
                    "latency probe: buttons {:#06x} polled frame {} -> physics tick {} (+{:.1} ms) -> first drawn frame {} (+{:.1} ms, {:.0}%) -> fully drawn frame {} (+{:.1} ms); mean {:.1} ms over {}",
                    press.buttons, press.frame, tick, ms(press.polled, ticked), seen, ms(press.polled, seen_at),
                    seen_weight * 100., frame, full, self.total_ms / self.count as f64, self.count));
                press.frame = 0;
            }
        }
        self.pending.retain(|p| p.frame != 0);
        lines
    }
}

fn poll(mut probe: ResMut<Probe>, input: Res<crate::input::ControllerInput>) {
    probe.poll(Instant::now(), input.raw_input().buttons);
}

fn tick(mut probe: ResMut<Probe>, presentation: Res<crate::presentation::Presentation>) {
    probe.tick(Instant::now(), presentation.generation());
}

fn present(mut probe: ResMut<Probe>, presentation: Res<crate::presentation::Presentation>,
    time: Res<Time<Fixed>>, replay: Res<crate::replay::Replay>, menu: Option<Res<crate::graphics_menu::Menu>>) {
    if replay.active { return; }
    let alpha = presentation.alpha(time.overstep_fraction());
    for line in probe.present(Instant::now(), alpha) {
        info!("{line} (smoothing {}, {})", if presentation.smoothing { "On" } else { "Off" },
            menu.as_ref().map_or("no menu".into(), |m| m.fps_limit_text()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }
    #[test]
    fn smoothing_on_draws_the_press_partly_then_fully() {
        let start = Instant::now();
        let mut probe = Probe::default();
        probe.poll(start, 0);
        probe.tick(at(start, 1), 1);
        assert!(probe.present(at(start, 2), 0.5).is_empty());
        // A pressed at frame 2; this frame has no tick (90 fps vs 60 Hz).
        probe.poll(at(start, 11), 0x1000);
        assert!(probe.present(at(start, 12), 0.9).is_empty());
        // Frame 3: tick 2 consumes it and is drawn at 40%.
        probe.poll(at(start, 22), 0x1000);
        probe.tick(at(start, 23), 2);
        assert!(probe.present(at(start, 24), 0.4).is_empty());
        // Frame 4: tick 3 makes tick 2 the previous snapshot, so it is fully drawn.
        probe.poll(at(start, 33), 0x1000);
        probe.tick(at(start, 34), 3);
        let lines = probe.present(at(start, 35), 0.1);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("polled frame 2 -> physics tick 2 (+12.0 ms) -> first drawn frame 3 (+13.0 ms, 40%) -> fully drawn frame 4 (+24.0 ms)"), "{}", lines[0]);
        // Holding the button is not a new press.
        probe.poll(at(start, 44), 0x1000);
        probe.tick(at(start, 45), 4);
        assert!(probe.present(at(start, 46), 0.5).is_empty());
        assert!(probe.pending.is_empty());
    }
    #[test]
    fn smoothing_off_is_fully_drawn_on_the_tick_frame() {
        let start = Instant::now();
        let mut probe = Probe::default();
        probe.poll(start, 0x0001);
        probe.tick(at(start, 2), 1);
        let lines = probe.present(at(start, 3), 1.);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("first drawn frame 1 (+3.0 ms, 100%) -> fully drawn frame 1 (+3.0 ms); mean 3.0 ms over 1"), "{}", lines[0]);
    }
    #[test]
    fn presses_that_never_reach_physics_expire() {
        let start = Instant::now();
        let mut probe = Probe::default();
        probe.poll(start, 0x0010);
        probe.poll(at(start, 2500), 0);
        assert!(probe.pending.is_empty());
    }
}
