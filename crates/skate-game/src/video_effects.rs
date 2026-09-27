//! Optional video effects beyond the original game (SK-051), shown in the pause menu's
//! "Video effects" page and saved inside settings/graphics.json. All default Off, so the
//! stock look stays the default. To add an effect: a field in `VideoEffects`, one `ROWS`
//! entry, its clamp in `validated`, its name in `apply_overrides` and its camera
//! components in `sync`.
use crate::option_rows::{count_step, level_name, on_off, OptionRow};
use bevy::{
    core_pipeline::prepass::{DepthPrepass, NormalPrepass},
    pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel},
    post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter},
    prelude::*,
    render::view::Hdr,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct VideoEffects {
    /// 0 Off, 1 Low, 2 Medium, 3 High (SK-054).
    pub bloom: u32,
    /// 0 Off, 1 Low, 2 Medium, 3 High, 4 Ultra (SK-052). Forces MSAA off while on.
    pub ssao: u32,
    /// Screen-space reflections on the retail reflective surfaces (SK-053).
    pub ssr: bool,
}

const LEVELS: &[&str] = &["Off", "Low", "Medium", "High", "Ultra"];

pub(crate) const ROWS: &[OptionRow<VideoEffects>] = &[
    OptionRow {
        label: "Bloom",
        value: |e| level_name(e.bloom, &LEVELS[..4]),
        change: |e, step| e.bloom = count_step(e.bloom, step, 3),
    },
    OptionRow {
        label: "Ambient occlusion",
        value: |e| if e.ssao == 0 { "Off".into() } else { format!("{} (MSAA off)", level_name(e.ssao, LEVELS)) },
        change: |e, step| e.ssao = count_step(e.ssao, step, 4),
    },
    OptionRow {
        label: "Screen reflections",
        value: |e| on_off(e.ssr),
        change: |e, _| e.ssr = !e.ssr,
    },
];

impl VideoEffects {
    pub(crate) fn validated(mut self) -> Self {
        self.bloom = self.bloom.min(3);
        self.ssao = self.ssao.min(4);
        self
    }
    /// `SKATE_VIDEO_FX=name=level,...` (e.g. `bloom=2,ssao=1`) for reproducible A/B runs;
    /// does not touch graphics.json. Unknown names or values are ignored.
    pub(crate) fn apply_overrides(&mut self, spec: &str) {
        for (name, value) in spec.split(',').filter_map(|pair| pair.trim().split_once('=')) {
            let Ok(value) = value.trim().parse::<u32>() else { continue };
            match name.trim() {
                "bloom" => self.bloom = value,
                "ssao" => self.ssao = value,
                "ssr" => self.ssr = value != 0,
                _ => warn!("SKATE_VIDEO_FX: unknown effect {name}"),
            }
        }
        *self = std::mem::take(self).validated();
        info!("SKATE_VIDEO_FX: {self:?}");
    }
}

/// Bloom on the HDR scene before the retail tone curve (SK-054). Bevy's Bloom node runs
/// before Tonemapping, and RetailTone (retail_exposure.rs) runs after it, so the glow is
/// tone-mapped with the scene like the recomp's `ToneMapScene(x + bloom)`. Additive. Low is
/// the recomp's threshold 0.9 / knee 0.08: sunlit surfaces (0.5-0.65 before tone) stay clean
/// and only sky, sun, lamps and neon glow; Medium/High lower the threshold for a stronger look.
fn bloom(level: u32) -> Option<Bloom> {
    let (intensity, threshold) = *[(0., 0.), (0.08, 0.9), (0.15, 0.8), (0.3, 0.7)].get(level as usize)?;
    (intensity > 0.).then_some(Bloom {
        intensity,
        prefilter: BloomPrefilter { threshold, threshold_softness: 0.09 },
        composite_mode: BloomCompositeMode::Additive,
        ..Bloom::NATURAL
    })
}

/// Bevy's SSAO (SK-052). It needs depth + normal prepasses (retail_depth.wgsl writes normals)
/// and MSAA off; retail_world.wgsl applies 60% of it on top of the baked lightmaps.
fn ssao(level: u32) -> Option<ScreenSpaceAmbientOcclusion> {
    use ScreenSpaceAmbientOcclusionQualityLevel::*;
    let quality_level = [None, Some(Low), Some(Medium), Some(High), Some(Ultra)].get(level as usize).copied().flatten()?;
    Some(ScreenSpaceAmbientOcclusion { quality_level, ..default() })
}

/// Share of the SSAO applied on retail surfaces (their lightmaps already hold baked occlusion).
/// `SKATE_SSAO_STRENGTH=0..1` overrides the default 0.6 for tuning.
fn ssao_strength() -> f32 {
    static VALUE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| std::env::var("SKATE_SSAO_STRENGTH").ok().and_then(|v| v.parse().ok())
        .filter(|v: &f32| (0.0..=1.0).contains(v)).unwrap_or(0.6))
}
/// `SKATE_SSAO_DEBUG=1` draws the raw occlusion in grey on retail surfaces.
fn ssao_debug() -> bool {
    static VALUE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| std::env::var_os("SKATE_SSAO_DEBUG").is_some_and(|v| v == "1"))
}

/// Keeps the gameplay camera's effect components in line with the menu. Runs every frame after
/// `graphics_menu::apply` and only touches the camera when something differs, so a map switch
/// that swaps Hdr/RetailTone (camera::set_world_environment) or an MSAA/occlusion change from
/// the menu is re-applied on the next frame.
pub(crate) fn sync(
    mut commands: Commands,
    menu: Option<Res<crate::graphics_menu::Menu>>,
    frame: Option<ResMut<crate::retail_render::ShadowState>>,
    retail_materials: Option<ResMut<Assets<crate::retail_render::RetailWorldMaterial>>>,
    mut ssr_applied: Local<Option<(bool, usize)>>,
    mut cameras: Query<(Entity, &mut Msaa, Option<&Bloom>, Option<&ScreenSpaceAmbientOcclusion>,
        Has<Hdr>, Has<crate::retail_render::RetailTone>, Has<DepthPrepass>, Has<NormalPrepass>),
        With<crate::camera::GameplayCamera>>,
) {
    let Some(menu) = menu else { return };
    let (effects, samples, occlusion) = menu.video_effects();
    if let Some(mut frame) = frame {
        // Retail SSAO strength and debug view, read by retail_world.wgsl (clock.z/w).
        let wanted = Vec2::new(ssao_strength(), if ssao_debug() { 1. } else { 0. });
        if frame.1.zw() != wanted {
            frame.1.z = wanted.x;
            frame.1.w = wanted.y;
        }
    }
    if let Some(mut materials) = retail_materials {
        // Re-run after a map load too: new materials read SSR_ENABLED, but also catch a toggle
        // that raced the load. Counting assets is cheap; the scan only runs when it changes.
        let state = (effects.ssr, materials.len());
        if *ssr_applied != Some(state) {
            let changed = crate::retail_render::set_ssr(&mut materials, effects.ssr);
            if changed > 0 { info!("SSR: {} on {changed} reflective materials", effects.ssr); }
            *ssr_applied = Some(state);
        }
    }
    for (entity, mut msaa, current_bloom, current_ssao, hdr, retail, depth, normal) in &mut cameras {
        let mut camera = commands.entity(entity);
        match (bloom(effects.bloom), current_bloom) {
            (Some(wanted), current) => {
                // Bloom needs Hdr; a procedural world drops Hdr on map switch, so add it back.
                if !hdr || current.is_none_or(|c| c.intensity != wanted.intensity) {
                    camera.insert((wanted, Hdr));
                }
            }
            (None, Some(_)) => {
                camera.remove::<Bloom>();
                // Retail worlds keep Hdr for their own tone curve.
                if !retail { camera.remove::<Hdr>(); }
            }
            (None, None) => {}
        }
        match (ssao(effects.ssao), current_ssao) {
            (Some(wanted), current) => {
                if current != Some(&wanted) || !depth || !normal {
                    camera.insert((wanted, DepthPrepass, NormalPrepass));
                }
                if *msaa != Msaa::Off { *msaa = Msaa::Off; }
            }
            (None, Some(_)) => {
                camera.remove::<(ScreenSpaceAmbientOcclusion, NormalPrepass)>();
                // Occlusion culling and SSR keep their own depth prepass.
                if !occlusion && !effects.ssr { camera.remove::<DepthPrepass>(); }
                *msaa = crate::graphics_menu::msaa(samples);
            }
            (None, None) => {}
        }
        // SSR raymarches the depth prepass; without it the shader falls back to the cube.
        if effects.ssr && !depth { camera.insert(DepthPrepass); }
        if !effects.ssr && effects.ssao == 0 && !occlusion && depth && current_ssao.is_none() {
            camera.remove::<DepthPrepass>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app(effects: VideoEffects) -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(crate::graphics_menu::Menu::for_tests(effects))
            .add_systems(Update, sync);
        let camera = app.world_mut().spawn((crate::camera::GameplayCamera, Camera3d::default(), Msaa::Sample4)).id();
        (app, camera)
    }
    fn set(app: &mut App, effects: VideoEffects) {
        app.world_mut().resource_mut::<crate::graphics_menu::Menu>().set_video_effects(effects);
        app.update();
    }
    #[test]
    fn missing_fields_default_off_and_overrides_clamp() {
        let effects: VideoEffects = serde_json::from_str("{}").unwrap();
        assert_eq!(effects, VideoEffects::default());
        let effects: VideoEffects = serde_json::from_str(r#"{"bloom":2}"#).unwrap();
        assert_eq!(effects, VideoEffects { bloom: 2, ..default() });
        let mut effects = VideoEffects::default();
        effects.apply_overrides("bloom=9, nothing=1, bloom2=x, ssao=7");
        assert_eq!(effects, VideoEffects { bloom: 3, ssao: 4, ssr: false });
        effects.apply_overrides("ssr=1");
        assert!(effects.ssr);
        assert_eq!((ROWS[0].value)(&VideoEffects::default()), "Off");
        assert_eq!((ROWS[1].value)(&VideoEffects { ssao: 2, ..default() }), "Medium (MSAA off)");
    }
    #[test]
    fn bloom_adds_and_removes_with_hdr_on_procedural_worlds() {
        let (mut app, camera) = app(VideoEffects { bloom: 2, ..default() });
        app.update();
        assert!(app.world().entity(camera).contains::<Bloom>());
        assert!(app.world().entity(camera).contains::<Hdr>());
        // A procedural map switch drops Hdr; the next frame puts it back.
        app.world_mut().entity_mut(camera).remove::<Hdr>();
        app.update();
        assert!(app.world().entity(camera).contains::<Hdr>());
        set(&mut app, VideoEffects::default());
        assert!(!app.world().entity(camera).contains::<Bloom>());
        assert!(!app.world().entity(camera).contains::<Hdr>());
    }
    #[test]
    fn retail_camera_keeps_hdr_when_bloom_turns_off() {
        let (mut app, camera) = app(VideoEffects { bloom: 1, ..default() });
        app.world_mut().entity_mut(camera).insert((Hdr, crate::retail_render::RetailTone::default()));
        app.update();
        set(&mut app, VideoEffects::default());
        assert!(!app.world().entity(camera).contains::<Bloom>());
        assert!(app.world().entity(camera).contains::<Hdr>());
    }
    #[test]
    fn ssao_turns_msaa_off_and_restores_it() {
        let (mut app, camera) = app(VideoEffects { ssao: 3, ..default() });
        app.update();
        let entity = app.world().entity(camera);
        assert!(entity.contains::<ScreenSpaceAmbientOcclusion>() && entity.contains::<NormalPrepass>() && entity.contains::<DepthPrepass>());
        assert_eq!(*entity.get::<Msaa>().unwrap(), Msaa::Off);
        set(&mut app, VideoEffects::default());
        let entity = app.world().entity(camera);
        assert!(!entity.contains::<ScreenSpaceAmbientOcclusion>() && !entity.contains::<NormalPrepass>());
        // Menu default: occlusion culling on, so its depth prepass stays; MSAA back to 4x.
        assert!(entity.contains::<DepthPrepass>());
        assert_eq!(*entity.get::<Msaa>().unwrap(), Msaa::Sample4);
    }
}
