//! Sound framework (SK-021). Setup decodes the original banks to assets/private/audio with an
//! `audio.json` index; `events.json` maps named events to those sounds.
//! - One-shots: write a `PlaySound` message with the event name. An event's `layers` play with
//!   it, like the original game's stacked samples (pop = tail + trucks + air): a layer is another
//!   event (its own variations and volume) or a sound id (played at the event's volume).
//! - Loops: call `SoundLoops::set(key, event, volume, pitch)` every frame the loop should play;
//!   a loop that is not set in a frame stops. Volume and pitch follow the latest call.
//! Channel volumes (master, music, effects) come from the Game options page.
//! Without decoded audio (setup skipped or failed) every call is a silent no-op.
use bevy::audio::{AudioSink, AudioSinkPlayback, PlaybackMode, Volume};
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;

mod gameplay;
mod menu;
pub(crate) use menu::MenuCursor;

const EVENTS: &str = include_str!("events.json");

pub(crate) struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>()
            .init_resource::<SoundLoops>()
            .add_systems(Startup, (load, smoke_test).chain())
            .add_systems(Update, (session_marker, gameplay::state_sounds, gameplay::loop_sounds, menu::pause_menu_sounds, play, reconcile_loops).chain());
    }
}

/// Play one sound for `event` (a key of events.json).
#[derive(Message, Clone, Debug)]
pub(crate) struct PlaySound(pub String);

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Channel {
    #[default]
    Effects,
    Music,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct EventDef {
    pub sounds: Vec<String>,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default)]
    pub volume_jitter: f32,
    #[serde(default)]
    pub pitch_jitter: f32,
    #[serde(default)]
    pub channel: Channel,
    /// Events or sound ids played at the same time (one level deep).
    #[serde(default)]
    pub layers: Vec<String>,
}
fn one() -> f32 {
    1.0
}

/// Decoded sounds (id -> asset path) plus the event table.
#[derive(Resource, Default)]
pub(crate) struct SoundLibrary {
    files: HashMap<String, String>,
    events: HashMap<String, EventDef>,
    seed: u32,
}

/// What to play for one event request.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Resolved {
    pub file: String,
    pub volume: f32,
    pub speed: f32,
    pub channel: Channel,
}

#[derive(Deserialize)]
struct Manifest {
    sounds: Vec<ManifestSound>,
}
#[derive(Deserialize)]
struct ManifestSound {
    id: String,
    file: String,
}

impl SoundLibrary {
    pub fn new(manifest: &str, events: &str) -> Result<Self, String> {
        let manifest: Manifest = serde_json::from_str(manifest).map_err(|e| format!("audio.json: {e}"))?;
        let events: HashMap<String, serde_json::Value> =
            serde_json::from_str(events).map_err(|e| format!("events.json: {e}"))?;
        let events = events
            .into_iter()
            .filter(|(name, _)| !name.starts_with('_'))
            .map(|(name, value)| {
                serde_json::from_value(value).map(|def| (name.clone(), def)).map_err(|e| format!("event {name}: {e}"))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            files: manifest.sounds.into_iter().map(|s| (s.id, s.file)).collect(),
            events,
            seed: 0x9e37_79b9,
        })
    }
    /// Picks a sound and its jittered volume/pitch; None if the event or its sounds are unknown.
    /// The event and its layers, each resolved once; layers of layers are ignored.
    pub fn resolve_layered(&mut self, event: &str) -> Vec<Resolved> {
        let Some(def) = self.events.get(event).cloned() else { return Vec::new() };
        let mut sounds: Vec<Resolved> = self.resolve(event).into_iter().collect();
        for layer in &def.layers {
            if self.events.contains_key(layer) {
                sounds.extend(self.resolve(layer));
            } else if let Some(file) = self.files.get(layer) {
                sounds.push(Resolved { file: file.clone(), volume: def.volume, speed: 1.0, channel: def.channel });
            }
        }
        sounds
    }
    pub fn resolve(&mut self, event: &str) -> Option<Resolved> {
        let def = self.events.get(event)?;
        let available: Vec<&String> = def.sounds.iter().filter(|id| self.files.contains_key(*id)).collect();
        if available.is_empty() {
            return None;
        }
        let mut next = || {
            // xorshift32: cheap, deterministic for tests.
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 17;
            self.seed ^= self.seed << 5;
            self.seed as f32 / u32::MAX as f32
        };
        let pick = available[(next() * available.len() as f32) as usize % available.len()];
        let jitter = |amount: f32, r: f32| 1.0 + amount * (r * 2.0 - 1.0);
        let (a, b) = (next(), next());
        Some(Resolved {
            file: self.files[pick].clone(),
            volume: (def.volume * jitter(def.volume_jitter, a)).max(0.0),
            speed: jitter(def.pitch_jitter, b).max(0.05),
            channel: def.channel,
        })
    }
}

/// Immediate-mode loops, keyed by caller-chosen names.
#[derive(Resource, Default)]
pub(crate) struct SoundLoops {
    wanted: HashMap<String, (String, f32, f32)>,
    playing: HashMap<String, (String, Entity)>,
}
impl SoundLoops {
    /// Keep `key` playing `event` this frame at `volume` (0..2) and `pitch` (playback speed).
    pub fn set(&mut self, key: &str, event: &str, volume: f32, pitch: f32) {
        self.wanted.insert(key.to_owned(), (event.to_owned(), volume, pitch));
    }
}

fn load(mut commands: Commands, config: Res<crate::config::Config>) {
    let path = config.asset_root.join("private/audio/audio.json");
    let library = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|manifest| SoundLibrary::new(&manifest, EVENTS));
    match library {
        Ok(library) => {
            info!("Audio: {} decoded sounds, {} events", library.files.len(), library.events.len());
            commands.insert_resource(library);
        }
        Err(error) => {
            info!("Audio disabled ({}): {error}", path.display());
            commands.insert_resource(SoundLibrary::default());
        }
    }
}

/// `SKATE_AUDIO_TEST=<event>` plays that event once at startup (manual/automated checks).
fn smoke_test(mut sounds: MessageWriter<PlaySound>) {
    if let Ok(event) = std::env::var("SKATE_AUDIO_TEST") {
        info!("Audio smoke test: {event}");
        sounds.write(PlaySound(event));
    }
}

fn channel_volume(options: &crate::game_options::GameOptions, channel: Channel) -> f32 {
    let channel = match channel {
        Channel::Effects => options.effects_volume,
        Channel::Music => options.music_volume,
    };
    options.master_volume as f32 / 100.0 * channel as f32 / 100.0
}

fn play(
    mut commands: Commands,
    mut requests: MessageReader<PlaySound>,
    mut library: ResMut<SoundLibrary>,
    options: Res<crate::game_options::GameOptions>,
    assets: Res<AssetServer>,
) {
    for PlaySound(event) in requests.read() {
        for sound in library.resolve_layered(event) {
            let volume = sound.volume * channel_volume(&options, sound.channel);
            commands.spawn((
                Name::new(format!("Sound {event}")),
                AudioPlayer::new(assets.load(sound.file)),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)).with_speed(sound.speed),
            ));
        }
    }
}

fn reconcile_loops(
    mut commands: Commands,
    mut loops: ResMut<SoundLoops>,
    mut library: ResMut<SoundLibrary>,
    options: Res<crate::game_options::GameOptions>,
    assets: Res<AssetServer>,
    mut sinks: Query<&mut AudioSink>,
) {
    let loops = &mut *loops;
    let wanted = std::mem::take(&mut loops.wanted);
    loops.playing.retain(|key, (event, entity)| {
        let keep = wanted.get(key).is_some_and(|(e, ..)| e == event);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
    for (key, (event, volume, pitch)) in wanted {
        let channel = library.events.get(&event).map_or(Channel::Effects, |d| d.channel);
        let volume = volume * channel_volume(&options, channel);
        if let Some((_, entity)) = loops.playing.get(&key) {
            if let Ok(mut sink) = sinks.get_mut(*entity) {
                sink.set_volume(Volume::Linear(volume));
                sink.set_speed(pitch);
            }
            continue;
        }
        let Some(sound) = library.resolve(&event) else { continue };
        let entity = commands
            .spawn((
                Name::new(format!("Sound loop {key}")),
                AudioPlayer::new(assets.load(sound.file)),
                PlaybackSettings { mode: PlaybackMode::Loop, ..default() }
                    .with_volume(Volume::Linear(volume))
                    .with_speed(pitch),
            ))
            .id();
        loops.playing.insert(key, (event, entity));
    }
}

/// First consumer: the session marker's AEMS event hashes, as `session_marker/<hash>` events.
/// The hash -> original sound mapping is not known yet, so these stay silent until
/// events.json gains entries for them.
fn session_marker(
    mut markers: MessageReader<crate::session_marker::SessionMarkerAudio>,
    mut sounds: MessageWriter<PlaySound>,
) {
    for marker in markers.read() {
        sounds.write(PlaySound(format!("session_marker/{:016x}", marker.0)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{"version":1,"sounds":[
        {"id":"GRINDS/1","file":"private/audio/GRINDS/1.wav"},
        {"id":"GRINDS/2","file":"private/audio/GRINDS/2.wav"}]}"#;

    #[test]
    fn events_resolve_to_available_sounds_with_bounded_jitter() {
        let events = r#"{"_comment":"x",
            "grind":{"sounds":["GRINDS/1","GRINDS/2","MISSING/1"],"volume":0.5,"volume_jitter":0.2,"pitch_jitter":0.1},
            "music":{"sounds":["GRINDS/1"],"channel":"music"},
            "gone":{"sounds":["MISSING/1"]}}"#;
        let mut library = SoundLibrary::new(MANIFEST, events).unwrap();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..50 {
            let sound = library.resolve("grind").unwrap();
            assert!(sound.file.starts_with("private/audio/GRINDS/"));
            assert!((0.4..=0.6).contains(&sound.volume), "{}", sound.volume);
            assert!((0.9..=1.1).contains(&sound.speed), "{}", sound.speed);
            seen.insert(sound.file);
        }
        assert_eq!(seen.len(), 2, "both available variations get picked");
        assert_eq!(library.resolve("music").unwrap().channel, Channel::Music);
        assert!(library.resolve("gone").is_none());
        assert!(library.resolve("unknown").is_none());
    }

    #[test]
    fn layers_play_with_their_event_one_level_deep() {
        let events = r#"{
            "pop":{"sounds":["GRINDS/1"],"volume":0.5,"layers":["board","GRINDS/1","MISSING/1"]},
            "board":{"sounds":["GRINDS/2"],"layers":["pop"]}}"#;
        let mut library = SoundLibrary::new(MANIFEST, events).unwrap();
        let sounds = library.resolve_layered("pop");
        let files: Vec<&str> = sounds.iter().map(|s| s.file.as_str()).collect();
        assert_eq!(files, ["private/audio/GRINDS/1.wav", "private/audio/GRINDS/2.wav", "private/audio/GRINDS/1.wav"]);
        assert_eq!(sounds[2].volume, 0.5, "a sound-id layer uses the event's volume");
        assert!(library.resolve_layered("unknown").is_empty());
    }

    #[test]
    fn shipped_event_table_parses() {
        let library = SoundLibrary::new(MANIFEST, EVENTS).unwrap();
        for (name, def) in &library.events {
            for layer in &def.layers {
                assert!(library.events.contains_key(layer) || layer.contains('/'), "{name}: unknown layer {layer}");
            }
        }
    }

    #[test]
    fn channel_volume_multiplies_master() {
        let mut options = crate::game_options::GameOptions::default();
        (options.master_volume, options.effects_volume) = (50, 80);
        assert!((channel_volume(&options, Channel::Effects) - 0.4).abs() < 1e-6);
    }
}
