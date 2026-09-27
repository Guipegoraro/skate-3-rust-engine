//! Optional video effects beyond the original game (SK-051), shown in the pause menu's
//! "Video effects" page and saved inside settings/graphics.json. All default Off, so the
//! stock look stays the default. To add an effect: a field in `VideoEffects`, one `ROWS`
//! entry, its clamp in `validated` and its camera components in `sync`.
use crate::option_rows::OptionRow;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct VideoEffects {}

pub(crate) const ROWS: &[OptionRow<VideoEffects>] = &[];

impl VideoEffects {
    pub(crate) fn validated(self) -> Self {
        self
    }
    /// `SKATE_VIDEO_FX=name=level,...` (e.g. `bloom=2`) for reproducible A/B runs; does not
    /// touch graphics.json. Unknown names or values are ignored.
    pub(crate) fn apply_overrides(&mut self, spec: &str) {
        for (name, value) in spec.split(',').filter_map(|pair| pair.trim().split_once('=')) {
            let Ok(_value) = value.trim().parse::<u32>() else { continue };
            match name.trim() {
                _ => warn!("SKATE_VIDEO_FX: unknown effect {name}"),
            }
        }
        *self = std::mem::take(self).validated();
    }
}

/// Keeps the gameplay camera's effect components in line with the menu. Runs every frame and
/// only touches the camera when something differs, so a map switch that swaps Hdr/RetailTone
/// (camera::set_world_environment) or an MSAA/occlusion change is re-applied on the next frame.
pub(crate) fn sync(menu: Option<Res<crate::graphics_menu::Menu>>) {
    let Some(_menu) = menu else { return };
}
