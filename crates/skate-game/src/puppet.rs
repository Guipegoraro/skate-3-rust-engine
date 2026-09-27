//! Puppets: extra rendered characters driven by stock-skeleton bone poses (SK-034).
//! Shared by multiplayer remote skaters and NPC pedestrians. A caller spawns a `Look` with
//! `PuppetLoader::spawn`, polls it until `LoadState::Ready` gives the joint bindings, shows it
//! with `PuppetLoader::show`, then writes one model-space pose per frame with `pose_locals`.
//! No networking here: the caller decides where poses come from.
use crate::customiser_material::SkaterMaterial;
use crate::physics::SkaterRuntime;
use bevy::{
    ecs::system::SystemParam,
    mesh::{morph::MorphWeights, skinning::SkinnedMesh},
    prelude::*,
    scene::SceneInstance,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};

/// Which character model a puppet wears. Also the multiplayer appearance message.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum Look {
    Stock,
    Outfit(Value),
    Native(String),
    Imported(String),
}

/// Marker on every puppet root; retail character materials apply below it.
#[derive(Component)]
pub(crate) struct Puppet;

/// A joint entity, the stock bone it follows and that bone's bound parent bone.
pub(crate) type Binding = (Entity, usize, Option<usize>);

#[derive(Component)]
struct OutfitPiece {
    id: String,
    mid: String,
}

/// Stock skeleton shared by all puppets: rest locals, parents and the board bone.
#[derive(Resource)]
pub(crate) struct PuppetRig {
    rest: Vec<Mat4>,
    parents: Vec<i32>,
    pub board: Option<usize>,
}
impl PuppetRig {
    fn new(app: &App) -> Self {
        let idle = app
            .world()
            .resource::<crate::assets::AssetManifest>()
            .0
            .initial_animation
            .clone();
        let skater = app.world().resource::<SkaterRuntime>();
        let evaluator = &skater.animation.evaluator;
        let pose = evaluator
            .evaluate(&[
                skate_core::animation::playback_tree::PoseCommand::Clip {
                    name: idle,
                    previous_time: 0.,
                    time: 0.,
                    loops: 0,
                },
                skate_core::animation::playback_tree::PoseCommand::Pose {
                    name: "RIG_TPOSE".into(),
                },
                skate_core::animation::playback_tree::PoseCommand::Add { motion_is_a: true },
            ])
            .unwrap_or_else(|_| skater.animation.pose.clone());
        Self {
            rest: pose
                .iter()
                .copied()
                .map(skate_core::animation::output::sqt_to_matrix)
                .map(crate::animation::native_matrix)
                .collect(),
            parents: evaluator.frames.parents.iter().map(|&p| p as i32).collect(),
            board: evaluator.frames.bone_names.iter().position(|n| n == "SKATEBOARD_ROOT"),
        }
    }

    /// Model-space matrices for every bone: the given ones, the rest filled in from the rest pose.
    pub(crate) fn globals(&self, known: impl IntoIterator<Item = (usize, Mat4)>) -> Vec<Mat4> {
        let mut result = vec![None; self.rest.len()];
        for (i, m) in known {
            if let Some(slot) = result.get_mut(i) {
                *slot = Some(m);
            }
        }
        fn visit(i: usize, rig: &PuppetRig, result: &mut [Option<Mat4>]) -> Mat4 {
            if let Some(m) = result[i] {
                return m;
            }
            let p = rig.parents[i];
            let matrix = if p >= 0 {
                visit(p as usize, rig, result) * rig.rest[i]
            } else {
                rig.rest[i]
            };
            result[i] = Some(matrix);
            matrix
        }
        for i in 0..result.len() {
            visit(i, self, &mut result);
        }
        result.into_iter().map(Option::unwrap).collect()
    }
}

// The GLB bone-local axes are rotated -90 degrees about X relative to the native frames.
fn basis() -> Mat4 {
    Mat4::from_cols(Vec4::X, -Vec4::Z, Vec4::Y, Vec4::W)
}

/// Joint-local transforms, in `bindings` order, for a model-space pose (native bone matrices).
pub(crate) fn pose_locals(bindings: &[Binding], globals: &[Mat4]) -> Vec<Transform> {
    let basis = basis();
    bindings
        .iter()
        .map(|&(_, i, parent)| {
            let global = globals.get(i).copied().unwrap_or(Mat4::IDENTITY) * basis;
            Transform::from_matrix(parent.map_or(global, |p| {
                (globals.get(p).copied().unwrap_or(Mat4::IDENTITY) * basis).inverse() * global
            }))
        })
        .collect()
}

/// A look being loaded under a hidden candidate entity.
pub(crate) struct PuppetLoad {
    pub root: Entity,
    scenes: Vec<Entity>,
    pub look: Look,
    started: Instant,
}
pub(crate) enum LoadState {
    Waiting,
    /// The candidate was despawned; the message says why.
    Failed(String),
    Ready(Vec<Binding>),
}

/// Everything needed to load, bind and show a puppet's model.
#[derive(SystemParam)]
pub(crate) struct PuppetLoader<'w, 's> {
    commands: Commands<'w, 's>,
    server: Res<'w, AssetServer>,
    parts: ResMut<'w, crate::customiser_parts::Parts>,
    models: Res<'w, crate::custom_models::CustomModels>,
    materials: ResMut<'w, Assets<SkaterMaterial>>,
    skater: Res<'w, SkaterRuntime>,
    spawner: Res<'w, SceneSpawner>,
    meshes: Query<'w, 's, (Entity, &'static SkinnedMesh)>,
    nodes: Query<'w, 's, (&'static Name, &'static Transform)>,
    parents: Query<'w, 's, &'static ChildOf>,
    instances: Query<'w, 's, &'static SceneInstance>,
    pieces: Query<'w, 's, &'static OutfitPiece>,
    morphs: Query<'w, 's, (Entity, &'static mut MorphWeights)>,
}
impl<'w, 's> PuppetLoader<'w, 's> {
    pub(crate) fn commands(&mut self) -> &mut Commands<'w, 's> {
        &mut self.commands
    }

    /// Start loading `look` as a hidden child of `parent`. Outfits that do not resolve fall
    /// back to the stock skater; None when the look has no loadable scene.
    pub(crate) fn spawn(&mut self, parent: Entity, look: Look) -> Option<PuppetLoad> {
        let look = match look {
            Look::Outfit(profile) => match self.parts.resolve(&profile) {
                Ok(p) => Look::Outfit(p),
                Err(e) => {
                    warn!("Puppet outfit: {e}");
                    Look::Stock
                }
            },
            other => other,
        };
        let mut specs = vec![];
        match &look {
            Look::Stock => specs.push(("private/skater.glb".to_owned(), None)),
            Look::Native(key) => specs.push((
                self.models
                    .online_native_path(key)
                    .unwrap_or_else(|| "private/skater.glb".to_owned()),
                None,
            )),
            Look::Imported(path) => specs.push((path.clone(), None)),
            Look::Outfit(profile) => {
                for v in profile["selections"]
                    .as_object()
                    .into_iter()
                    .flat_map(|s| s.values())
                {
                    let Some((id, mid)) = v["asset_id"].as_str().zip(v["material_id"].as_str())
                    else {
                        continue;
                    };
                    if let Some(part) = self.parts.library.models.get(id) {
                        specs.push((part.scene.clone(), Some((id.to_owned(), mid.to_owned()))));
                    }
                }
            }
        }
        if specs.is_empty() || specs.len() > 32 {
            return None;
        }
        let candidate = self
            .commands
            .spawn((Transform::default(), Visibility::Hidden, ChildOf(parent)))
            .id();
        if let Look::Native(key) = &look {
            self.commands
                .entity(candidate)
                .insert(crate::custom_models::NativeModelRoot(key.clone()));
        }
        if matches!(look, Look::Imported(_)) {
            self.commands
                .entity(candidate)
                .insert(crate::custom_models::CustomModelRoot);
        }
        let mut scenes = vec![];
        for (path, piece) in specs {
            let mut e = self.commands.spawn((
                SceneRoot(self.server.load(GltfAssetLabel::Scene(0).from_asset(path))),
                ChildOf(candidate),
            ));
            if let Some((id, mid)) = piece {
                e.insert((
                    crate::customiser_parts::PartRoot(id.clone()),
                    OutfitPiece { id, mid },
                ));
            }
            scenes.push(e.id());
        }
        Some(PuppetLoad {
            root: candidate,
            scenes,
            look,
            started: Instant::now(),
        })
    }

    /// Check a load. When ready, applies outfit materials/morphs and render layers and
    /// returns the joint bindings; `cull` keeps frustum culling on (off for remote skaters,
    /// whose poses can leave the bind-pose bounds). Failed loads are despawned.
    pub(crate) fn poll(&mut self, p: &PuppetLoad, cull: bool) -> LoadState {
        if p.started.elapsed() > Duration::from_secs(120) {
            self.commands.entity(p.root).despawn();
            return LoadState::Failed("loading timed out".into());
        }
        if p.scenes.iter().any(|&e| {
            !self
                .instances
                .get(e)
                .is_ok_and(|i| self.spawner.instance_is_ready(**i))
        }) {
            return LoadState::Waiting;
        }
        if let Look::Outfit(profile) = &p.look {
            for e in &p.scenes {
                if let Ok(piece) = self.pieces.get(*e) {
                    self.parts.warm(&piece.mid, &self.server, &mut self.materials);
                }
            }
            if !self.parts.tattoos_ready(profile, &self.server)
                || p.scenes.iter().any(|&e| {
                    self.pieces
                        .get(e)
                        .is_ok_and(|piece| !self.parts.material_ready(&piece.mid, &self.server))
                })
            {
                return LoadState::Waiting;
            }
        }
        let bindings = match crate::animation::AnimationStatus::for_scene(
            p.root,
            &self.skater.animation.evaluator.frames.bone_names,
            &self.meshes,
            &self.nodes,
            &self.parents,
        ) {
            Ok(b) => b.online_bindings(),
            Err(e) => {
                self.commands.entity(p.root).despawn();
                return LoadState::Failed(e);
            }
        };
        if let Look::Outfit(profile) = &p.look {
            for &scene in &p.scenes {
                let Ok(piece) = self.pieces.get(scene) else {
                    continue;
                };
                let Some(material) =
                    self.parts.profile_material(&piece.id, &piece.mid, profile, &self.materials)
                else {
                    continue;
                };
                let handle = self.materials.add(material);
                for (e, _) in &self.meshes {
                    if self.parents.iter_ancestors(e).any(|p| p == scene) {
                        self.commands
                            .entity(e)
                            .remove::<MeshMaterial3d<StandardMaterial>>()
                            .insert(MeshMaterial3d(handle.clone()));
                    }
                }
            }
            let weights: Vec<f32> = self
                .parts
                .library
                .morphs
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    profile["morphs"][n]
                        .as_f64()
                        .unwrap_or(if (2..19).contains(&i) { 0.25 } else { 0. })
                        .clamp(0., 0.5) as f32
                })
                .collect();
            for (e, mut m) in &mut self.morphs {
                if self.parents.iter_ancestors(e).any(|e| e == p.root)
                    && m.weights().len() == weights.len()
                {
                    m.weights_mut().copy_from_slice(&weights);
                }
            }
        }
        for (e, _) in &self.meshes {
            if self.parents.iter_ancestors(e).any(|e| e == p.root) {
                let mut entity = self.commands.entity(e);
                entity.insert(bevy::camera::visibility::RenderLayers::from_layers(&[0, 28]));
                if !cull {
                    entity.insert(bevy::camera::visibility::NoFrustumCulling);
                }
            }
        }
        LoadState::Ready(bindings)
    }

    /// Put the loaded model in `globals`'s pose and make it visible.
    pub(crate) fn show(&mut self, p: &PuppetLoad, bindings: &[Binding], globals: &[Mat4]) {
        for (&(entity, _, _), local) in bindings.iter().zip(pose_locals(bindings, globals)) {
            self.commands.entity(entity).insert(local);
        }
        self.commands.entity(p.root).insert(Visibility::Inherited);
    }
}

pub(crate) struct PuppetPlugin;
impl Plugin for PuppetPlugin {
    fn build(&self, app: &mut App) {
        let rig = PuppetRig::new(app);
        app.insert_resource(rig);
    }
}
