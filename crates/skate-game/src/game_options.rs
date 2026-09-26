//! Player-facing game options shown in the pause menu's "Game options" page.
//! To add an option: add a field to `GameOptions` and one entry to `ROWS`.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Resource, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct GameOptions {
    pub stick_indicator: bool,
    #[serde(skip)]
    path: PathBuf,
}
impl Default for GameOptions {
    fn default() -> Self {
        Self { stick_indicator: true, path: PathBuf::new() }
    }
}

/// One menu row: its label, how to show the current value, and how Left/Right/Enter change it.
pub(crate) struct OptionRow {
    pub label: &'static str,
    pub value: fn(&GameOptions) -> String,
    pub change: fn(&mut GameOptions, i32),
}

pub(crate) const ROWS: &[OptionRow] = &[OptionRow {
    label: "Right-stick indicator",
    value: |o| on_off(o.stick_indicator),
    change: |o, _| o.stick_indicator = !o.stick_indicator,
}];

fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.into()
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
