use super::appearance::{Appearances, Look, RemoteCharacter};
use super::*;
use crate::puppet::{Binding, LoadState, PuppetLoad, PuppetLoader, PuppetRig, pose_locals};
use skate_net::interpolation::{Buffer, Clock, position};
#[derive(Resource, Default)]
pub(super) struct RemoteSkins {
    reported: f64,
    actors: BTreeMap<u64, RemoteSkin>,
}
#[derive(Default)]
struct RemoteSkin {
    root: Option<Entity>,
    bindings: Vec<Binding>,
    visible: Option<Entity>,
    pending: Option<PuppetLoad>,
    requested: Option<[u8; 32]>,

    positions: Buffer<[f32; 3]>,
    roots: Buffer<Transform>,
    poses: Buffer<Vec<Transform>>,
    clock: Clock,
    epoch: u64,
}
pub(super) struct RemoteRenderPlugin;
impl Plugin for RemoteRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RemoteSkins>().add_systems(
            Update,
            (spawn, bind, present)
                .chain()
                .after(crate::modding::vehicles::present),
        );
    }
}
fn bone_globals(bones: &[skate_net::Bone], rig: &PuppetRig) -> Vec<Mat4> {
    rig.globals(bones.iter().map(|b| (b.index as usize, network::matrix(b.pose))))
}
pub(super) fn spawn(
    mut loader: PuppetLoader,
    net: Res<Multiplayer>,
    looks: Res<Appearances>,
    mut skins: ResMut<RemoteSkins>,
) {
    let removed: Vec<_> = skins
        .actors
        .keys()
        .filter(|id| !net.remotes.contains_key(id))
        .copied()
        .collect();
    for id in removed {
        if let Some(root) = skins.actors.remove(&id).and_then(|s| s.root) {
            loader.commands().entity(root).despawn();
        }
    }
    for &id in net.remotes.keys() {
        let skin = skins.actors.entry(id).or_insert_with(|| RemoteSkin {
            clock: Clock::for_connection(net.loopback),
            ..default()
        });
        let root = *skin.root.get_or_insert_with(|| {
            loader
                .commands()
                .spawn((
                    RemoteCharacter,
                    crate::puppet::Puppet,
                    Transform::default(),
                    Visibility::Inherited,
                    Name::new("Remote skater"),
                ))
                .id()
        });
        let compatible = net
            .lobby
            .as_ref()
            .and_then(|l| l.actors.get(&id))
            .is_some_and(|a| a.info.rig == net.info.rig);
        let (key, look) = if skin.visible.is_some() && compatible {
            looks
                .looks
                .get(&id)
                .cloned()
                .unwrap_or(([0; 32], Look::Stock))
        } else {
            ([0; 32], Look::Stock)
        };
        if skin.requested == Some(key) {
            continue;
        }
        skin.requested = Some(key);
        if let Some(old) = skin.pending.take() {
            loader.commands().entity(old.root).despawn();
        }
        skin.pending = loader.spawn(root, look);
    }
}

fn bind(
    mut loader: PuppetLoader,
    net: Res<Multiplayer>,
    rig: Res<PuppetRig>,
    mut skins: ResMut<RemoteSkins>,
) {
    for (id, skin) in skins.actors.iter_mut() {
        let Some(p) = skin.pending.as_ref() else {
            continue;
        };
        let bindings = match loader.poll(p, false) {
            LoadState::Waiting => continue,
            LoadState::Failed(e) => {
                warn!("Remote appearance rejected ({e}); previous skater retained");
                skin.pending = None;
                continue;
            }
            LoadState::Ready(bindings) => bindings,
        };
        let bones = net
            .remotes
            .get(id)
            .and_then(|r| r.poses.back())
            .map(|p| p.bones.as_slice())
            .unwrap_or(&[]);
        loader.show(p, &bindings, &bone_globals(bones, &rig));
        if let Some(old) = skin.visible.replace(p.root) {
            loader.commands().entity(old).despawn();
        }
        info!("ONLINE_CHARACTER_VISIBLE peer={id} joints={} kind={}", bindings.len(),
            if matches!(p.look, Look::Imported(_)) { "import" } else { "retail" });
        skin.bindings = bindings;
        skin.poses = Buffer::default();
        skin.pending = None;
    }
}

fn present(
    mut net: ResMut<Multiplayer>,
    vehicles: Res<crate::modding::vehicles::Vehicles>,
    rig: Res<PuppetRig>,
    mut skins: ResMut<RemoteSkins>,
    mut nodes: Query<&mut Transform>,
) {
    let now = net.started.elapsed().as_secs_f64();
    for (&id, remote) in &net.remotes {
        let Some(mut skin) = skins.actors.remove(&id) else {
            continue;
        };
        if skin.epoch != remote.epoch {
            skin.positions = Buffer::default();
            skin.roots = Buffer::default();
            skin.poses = Buffer::default();
            skin.clock = Clock::for_connection(net.loopback);
            skin.epoch = remote.epoch;
        }
        for sample in &remote.roots {
            if skin
                .roots
                .samples
                .back()
                .is_some_and(|p| p.time >= sample.captured)
            {
                continue;
            }
            if skin.roots.insert(
                sample.captured,
                Transform::from_matrix(network::matrix(sample.pose)),
            ) {
                skin.positions.insert(sample.captured, sample.pose.p);
                skin.clock.observe(0, sample.captured, sample.received);
            }
        }
        // Each animation sample is resolved once; physics arrivals never duplicate it.
        if !skin.bindings.is_empty() {
            for sample in &remote.poses {
                if skin
                    .poses
                    .samples
                    .back()
                    .is_some_and(|p| p.time >= sample.captured)
                {
                    continue;
                }
                let locals = pose_locals(&skin.bindings, &bone_globals(&sample.bones, &rig));
                if skin.poses.insert(sample.captured, locals) {
                    skin.clock.observe(1, sample.captured, sample.received);
                }
            }
        }
        if let (Some(root), Some(time)) = (skin.root, skin.clock.step(now)) {
            if let Some((a, b, alpha)) = skin.roots.pair(time) {
                let mut transform = crate::presentation::blend(
                    skin.roots.samples[a].value,
                    skin.roots.samples[b].value,
                    alpha,
                );
                if let Some(p) = position(&skin.positions, time) {
                    transform.translation = Vec3::from_array(p);
                }
                if let Ok(mut t) = nodes.get_mut(root) {
                    *t = transform;
                }
            }
            if let Some((a, b, alpha)) = skin.poses.pair(time) {
                let ag = &skin.poses.samples[a].value;
                let bg = &skin.poses.samples[b].value;
                for ((&(entity, _, _), a), b) in skin.bindings.iter().zip(ag).zip(bg) {
                    if let Ok(mut t) = nodes.get_mut(entity) {
                        *t = crate::presentation::blend(*a, *b, alpha);
                    }
                }
            }
        }
        if let Some(root) = skin.root {
            if let Some(attached) = crate::modding::vehicles::network::attached_root(&vehicles, id)
            {
                if let Ok(mut t) = nodes.get_mut(root) {
                    *t = attached;
                }
            }
        }
        let seated = remote.body.enabled & (1u64 << 62) != 0;
        for &(entity, i, _) in &skin.bindings {
            if rig.board == Some(i) {
                if let Ok(mut t) = nodes.get_mut(entity) {
                    t.scale = Vec3::splat(if seated { 0.001 } else { 1. });
                }
            }
        }
        skins.actors.insert(id, skin);
    }
    if now < skins.reported || now - skins.reported >= 1. {
        let delay = skins
            .actors
            .values()
            .map(|s| s.clock.delay)
            .fold(0., f64::max);
        let stalls: u64 = skins.actors.values().map(|s| s.clock.underruns).sum();
        net.visual_status = format!(
            "Visual interpolation: {:.0} ms target buffer | stalls {}",
            delay * 1000.,
            stalls
        );
        if net.active() {
            info!("MULTIPLAYER_INTERPOLATION {}", net.visual_status);
        }
        skins.reported = now;
    }
}

#[cfg(test)]
mod online_owned_tests {
    use super::*;
    use bevy::{mesh::skinning::SkinnedMesh, scene::SceneInstance};
    #[test]
    #[ignore = "requires SKATE3_ASSET_ROOT prepared owned assets"]
    fn online_appearance_owned_male_and_female_outfits_bind_every_clothing_rig() {
        use bevy::{
            asset::AssetPlugin, ecs::system::SystemState, gltf::GltfPlugin, image::ImagePlugin,
            mesh::MeshPlugin, scene::ScenePlugin,
        };
        let assets = std::path::PathBuf::from(std::env::var("SKATE3_ASSET_ROOT").unwrap());
        let banks = skate_data::animation_banks::AnimationBanks::load(&assets).unwrap();
        let names = skate_data::animation_frames::AnimationFrames::from_banks(&banks)
            .unwrap()
            .bone_names;
        let library: crate::customiser_parts::Library = serde_json::from_slice(
            &std::fs::read(
                crate::customiser_parts::asset_directory(&assets).join("library-v3.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let parts = crate::customiser_parts::Parts::for_test(library);
        let mut app = App::new();
        crate::custom_models::register_source(&mut app);
        app.add_plugins((
            MinimalPlugins,
            AssetPlugin {
                file_path: assets.to_string_lossy().into_owned(),
                ..default()
            },
            ImagePlugin::default(),
            MeshPlugin,
            ScenePlugin,
            GltfPlugin::default(),
        ));
        app.init_asset::<StandardMaterial>()
            .init_asset::<AnimationClip>()
            .register_type::<MeshMaterial3d<StandardMaterial>>();
        app.finish();
        app.cleanup();
        if let Ok(path) = std::env::var("SKATE_ONLINE_TEST_GLB") {
            let bytes = std::fs::read(path).unwrap();
            super::super::appearance::validate_glb(&bytes).unwrap();
            let cache = super::super::appearance::cache_directory();
            std::fs::create_dir_all(cache).unwrap();
            let mut payload = vec![0];
            payload.extend(bytes);
            let received = super::super::appearance_transfer::transfer_over_udp(payload);
            assert_eq!(received[0], 0);
            super::super::appearance::validate_glb(&received[1..]).unwrap();
            std::fs::write(cache.join("received-test.glb"), &received[1..]).unwrap();
            let handle = app
                .world()
                .resource::<AssetServer>()
                .load(GltfAssetLabel::Scene(0).from_asset("online-characters://received-test.glb"));
            let root = app
                .world_mut()
                .spawn((SceneRoot(handle), Transform::default(), Visibility::Hidden))
                .id();
            let started = Instant::now();
            loop {
                app.update();
                let w = app.world();
                if w.get::<SceneInstance>(root)
                    .is_some_and(|i| w.resource::<SceneSpawner>().instance_is_ready(**i))
                {
                    break;
                }
                assert!(
                    started.elapsed().as_secs() < 45,
                    "received online import failed to load"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            let mut queries: SystemState<(
                Query<(Entity, &SkinnedMesh)>,
                Query<(&Name, &Transform)>,
                Query<&ChildOf>,
            )> = SystemState::new(app.world_mut());
            let (meshes, nodes, parents) = queries.get(app.world());
            let animation = crate::animation::AnimationStatus::for_scene(
                root, &names, &meshes, &nodes, &parents
            ).unwrap();
            let joints=animation.online_bindings();
            assert!(!joints.is_empty());
            // A received model must accept poses before display, then keep moving.
            let first:Vec<_>=(0..names.len()).map(|i|Mat4::from_translation(Vec3::new(i as f32*0.01,1.,0.))).collect();
            for (joint,pose) in animation.pose_transforms(&first) {
                assert!(pose.to_matrix().is_finite());
                *app.world_mut().get_mut::<Transform>(joint).unwrap()=pose;
            }
            *app.world_mut().get_mut::<Visibility>(root).unwrap()=Visibility::Inherited;
            let second:Vec<_>=first.iter().enumerate().map(|(i,m)|*m*Mat4::from_rotation_z(0.01*(i+1) as f32)).collect();
            for (joint,pose) in animation.pose_transforms(&second) {
                assert!(pose.to_matrix().is_finite());
                assert_ne!(*app.world().get::<Transform>(joint).unwrap(),pose);
                *app.world_mut().get_mut::<Transform>(joint).unwrap()=pose;
            }
            app.world_mut().entity_mut(root).despawn();
            app.update();
            std::fs::remove_file(cache.join("received-test.glb")).unwrap();
        }
        for gender in ["male", "female"] {
            let profile = parts.resolve(&parts.library.defaults[gender]).unwrap();
            let candidate = app
                .world_mut()
                .spawn((Transform::default(), Visibility::Hidden))
                .id();
            let paths: Vec<_> = profile["selections"]
                .as_object()
                .unwrap()
                .values()
                .map(|v| {
                    parts.library.models[v["asset_id"].as_str().unwrap()]
                        .scene
                        .clone()
                })
                .collect();
            assert!(paths.len() > 3);
            let mut scenes = vec![];
            for path in paths {
                let handle = app
                    .world()
                    .resource::<AssetServer>()
                    .load(GltfAssetLabel::Scene(0).from_asset(path));
                scenes.push(
                    app.world_mut()
                        .spawn((SceneRoot(handle), ChildOf(candidate)))
                        .id(),
                );
            }
            let started = Instant::now();
            loop {
                app.update();
                let world = app.world();
                if scenes.iter().all(|&e| {
                    world
                        .get::<SceneInstance>(e)
                        .is_some_and(|i| world.resource::<SceneSpawner>().instance_is_ready(**i))
                }) {
                    break;
                }
                assert!(
                    started.elapsed().as_secs() < 45,
                    "{gender} scene loading timed out"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            let mut queries: SystemState<(
                Query<(Entity, &SkinnedMesh)>,
                Query<(&Name, &Transform)>,
                Query<&ChildOf>,
            )> = SystemState::new(app.world_mut());
            let (meshes, nodes, parents) = queries.get(app.world());
            let binding = crate::animation::AnimationStatus::for_scene(
                candidate, &names, &meshes, &nodes, &parents,
            )
            .unwrap()
            .online_bindings();
            let expected: std::collections::HashSet<_> = meshes
                .iter()
                .filter(|(e, _)| parents.iter_ancestors(*e).any(|e| e == candidate))
                .flat_map(|(_, m)| m.joints.iter().copied())
                .collect();
            assert_eq!(binding.len(), expected.len());
            for scene in scenes {
                assert!(
                    binding
                        .iter()
                        .any(|(e, _, _)| parents.iter_ancestors(*e).any(|e| e == scene)),
                    "all outfit scene rigs must animate"
                );
            }
            app.world_mut().entity_mut(candidate).despawn();
            app.update();
        }
    }
}
