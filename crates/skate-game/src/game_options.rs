//! Player-facing game options shown in the pause menu's "Game options" page.
//! To add an option: add a field to `GameOptions` and one entry to `ROWS`.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct GameOptions {
    pub stick_indicator: bool,
    /// B (or F3) on foot toggles fly mode.
    pub fly_mode: bool,
    /// Sound channel volumes in percent, 0..=100 (SK-021).
    pub master_volume: u32,
    pub music_volume: u32,
    pub effects_volume: u32,
    #[serde(skip)]
    path: PathBuf,
}
impl Default for GameOptions {
    fn default() -> Self {
        Self {
            stick_indicator: true,
            fly_mode: true,
            master_volume: 80,
            music_volume: 70,
            effects_volume: 100,
            path: PathBuf::new(),
        }
    }
}

/// One menu row: its label, how to show the current value, and how Left/Right/Enter change it.
pub(crate) struct OptionRow {
    pub label: &'static str,
    pub value: fn(&GameOptions) -> String,
    pub change: fn(&mut GameOptions, i32),
}

pub(crate) const ROWS: &[OptionRow] = &[
    OptionRow {
        label: "Right-stick indicator",
        value: |o| on_off(o.stick_indicator),
        change: |o, _| o.stick_indicator = !o.stick_indicator,
    },
    OptionRow {
        label: "Fly mode (B on foot)",
        value: |o| on_off(o.fly_mode),
        change: |o, _| o.fly_mode = !o.fly_mode,
    },
    OptionRow {
        label: "Master volume",
        value: |o| percent(o.master_volume),
        change: |o, step| o.master_volume = volume_step(o.master_volume, step),
    },
    OptionRow {
        label: "Music volume",
        value: |o| percent(o.music_volume),
        change: |o, step| o.music_volume = volume_step(o.music_volume, step),
    },
    OptionRow {
        label: "Effects volume",
        value: |o| percent(o.effects_volume),
        change: |o, step| o.effects_volume = volume_step(o.effects_volume, step),
    },
];

fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.into()
}

fn percent(value: u32) -> String {
    format!("{value}%")
}

/// Left/Right step by 10. Right/Enter/click past 100 wraps to 0, so a mouse alone can set it.
fn volume_step(value: u32, step: i32) -> u32 {
    if step > 0 && value >= 100 {
        return 0;
    }
    (value as i32 + step.signum() * 10).clamp(0, 100) as u32
}

impl GameOptions {
    fn load(path: PathBuf) -> Self {
        let options = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
                warn!("Game options: {e}");
                Self::default()
            }),
            Err(_) => Self::default(),
        };
        Self { path, ..options }
    }
    pub(crate) fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(self.path.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }
}

pub(crate) struct GameOptionsPlugin;
impl Plugin for GameOptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOptions>().add_systems(Startup, load);
    }
}

fn load(mut commands: Commands, config: Res<crate::config::Config>) {
    let root = config.asset_root.parent().unwrap_or(&config.asset_root);
    commands.insert_resource(GameOptions::load(root.join("settings/game-options.json")));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rows_toggle_and_missing_fields_use_defaults() {
        let mut options: GameOptions = serde_json::from_str("{}").unwrap();
        assert!(options.stick_indicator);
        (ROWS[0].change)(&mut options, 1);
        assert_eq!((ROWS[0].value)(&options), "Off");
    }
}
