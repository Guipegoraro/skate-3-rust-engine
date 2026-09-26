//! Original front-end textures exported by setup (tools/asset_pipeline/ui_textures.py).
//! Look one up by "<bundle>/<texture>", e.g. "hud2/trickanalyser/1". Missing or invalid
//! textures return None so overlays can fall back to drawn shapes.
use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Deserialize)]
struct Entry {
    file: String,
    width: u32,
    height: u32,
}

#[derive(Resource, Default)]
pub(crate) struct UiTextures {
    root: PathBuf,
    index: HashMap<String, Entry>,
    loaded: HashMap<String, Handle<Image>>,
}
impl UiTextures {
    pub(crate) fn get(&mut self, key: &str, images: &mut Assets<Image>) -> Option<Handle<Image>> {
        if let Some(handle) = self.loaded.get(key) {
            return Some(handle.clone());
        }
        let entry = self.index.get(key)?;
        let bytes = std::fs::read(self.root.join(&entry.file)).ok()?;
        if bytes.len() != entry.width as usize * entry.height as usize * 4 {
            warn!("UI texture {key}: unexpected size");
            return None;
        }
        let image = Image::new(
            Extent3d { width: entry.width, height: entry.height, depth_or_array_layers: 1 },
            TextureDimension::D2,
            bytes,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD,
        );
        let handle = images.add(image);
        self.loaded.insert(key.to_owned(), handle.clone());
        Some(handle)
    }
}

pub(crate) struct UiTexturesPlugin;
impl Plugin for UiTexturesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTextures>().add_systems(PreStartup, load);
    }
}

fn load(mut textures: ResMut<UiTextures>, config: Res<crate::config::Config>) {
    textures.root = config.asset_root.join("private/hud/ui");
    let path = textures.root.join("index.json");
    match std::fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(index) => textures.index = index,
            Err(e) => warn!("UI textures {}: {e}", path.display()),
        },
        Err(_) => info!("No original UI textures at {}; overlays use drawn shapes", path.display()),
    }
}
