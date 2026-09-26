//! On-foot fly/noclip mode (Game options "Fly mode"). B (or F3) on foot freezes the skater's
//! native physics, as a mod vehicle does, and hands the controller to a free camera. Pressing
//! it again teleports the skater on foot onto the ground below the camera, through the same
//! manual teleport path the vehicle exit uses. Disabled while multiplayer is active.
use crate::camera::GameplayCamera;
use bevy::prelude::*;
use skate_core::{math::Vector3, physics::triangle_query::{TriangleLineHit, triangle_segment}};

const B: u16 = 0x2000;
const LB: u16 = 0x0100;
const RB: u16 = 0x0200;
const LEFT_STICK_CLICK: u16 = 0x0040;
const SPEED: f32 = 8.0;
const FAST_SPEED: f32 = 32.0;
const GROUND_SEARCH: f32 = 500.0;
// On-foot physical category (BipedGround, BipedAir, pushing, landing on deck).
const ON_FOOT: u32 = 500;

#[derive(Resource, Default)]
pub(crate) struct FlyMode {
    active: bool,
    camera: Transform,
    previous_buttons: u16,
    release_controls: bool,
    notice: Option<(String, f32)>,
}
impl FlyMode {
    pub(crate) fn active(&self) -> bool {
        self.active
    }
    pub(crate) fn camera(&self) -> Option<Transform> {
        self.active.then_some(self.camera)
    }
    fn say(&mut self, text: &str) {
        self.notice = Some((text.into(), 2.5));
    }
}

pub(crate) struct FlyModePlugin;
impl Plugin for FlyModePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlyMode>()
            .add_systems(Startup, spawn_label)
            .add_systems(
                PreUpdate,
                controls.after(crate::input::poll_controllers).after(bevy::input::InputSystems),
            )
            .add_systems(Update, update_label);
    }
}

fn axis(value: f32) -> f32 {
    value.signum() * ((value.abs() - 0.18) / 0.82).clamp(0.0, 1.0)
}

fn controls(
    mut fly: ResMut<FlyMode>,
    options: Res<crate::game_options::GameOptions>,
    mut input: ResMut<crate::input::ControllerInput>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time<Real>>,
    menu: Option<Res<crate::graphics_menu::Menu>>,
    (replay, vehicles, transition): (
        Res<crate::replay::Replay>,
        Res<crate::modding::vehicles::Vehicles>,
        Res<crate::map_transition::MapTransition>,
    ),
    net: Option<Res<crate::multiplayer::Multiplayer>>,
    physics: Res<crate::physics::GamePhysics>,
    mut skater: ResMut<crate::physics::SkaterRuntime>,
    cameras: Query<&Transform, With<GameplayCamera>>,
) {
    let pad = input.raw_input();
    let pressed = pad.buttons & !fly.previous_buttons;
    fly.previous_buttons = pad.buttons;
    // Map changes, replay and multiplayer end the flight without teleporting.
    if fly.active && (transition.busy() || replay.active || net.as_ref().is_some_and(|n| n.active())) {
        fly.active = false;
    }
    let menu_open = !crate::graphics_menu::gameplay_active(menu);
    let toggle = !menu_open && (pressed & B != 0 || keys.just_pressed(KeyCode::F3));
    if toggle && fly.active {
        land(&mut fly, &physics, &mut skater);
    } else if toggle && options.fly_mode {
        let state = &skater.player_input.physical.state;
        let ready = state.category_12 == ON_FOOT
            && state.state_16 != 702
            && state.flag_69 == 0
            && skater.player_input.pending_teleport().is_none()
            && !physics.board_wiping_out
            && !replay.active
            && !vehicles.occupied()
            && !transition.busy();
        if net.as_ref().is_some_and(|n| n.active()) {
            fly.say("Voo indisponivel no multiplayer");
        } else if ready {
            if let Ok(camera) = cameras.single() {
                fly.camera = *camera;
                fly.active = true;
                fly.say("Voo ligado: B para descer");
            }
        }
    }
    // Like replay: gameplay never sees flight input, and after landing held controls
    // must be released before they can become a push, jump or flick.
    if fly.active || fly.release_controls {
        input.discard_gameplay();
        if !fly.active
            && pad.buttons == 0
            && pad.triggers.iter().all(|v| *v < 0.05)
            && pad.left.iter().chain(&pad.right).all(|v| v.abs() < 0.18)
        {
            fly.release_controls = false;
        }
    }
    if !fly.active || menu_open {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let camera = &mut fly.camera;
    let (mut yaw, mut pitch, _) = camera.rotation.to_euler(EulerRot::YXZ);
    yaw -= axis(pad.right[0]) * dt * 2.0;
    pitch = (pitch + axis(pad.right[1]) * dt * 2.0).clamp(-1.5, 1.5);
    camera.rotation = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
    let key = |code| f32::from(keys.pressed(code));
    let x = axis(pad.left[0]) + key(KeyCode::KeyD) - key(KeyCode::KeyA);
    let z = axis(pad.left[1]) + key(KeyCode::KeyW) - key(KeyCode::KeyS);
    let y = f32::from(pad.buttons & RB != 0) + key(KeyCode::KeyE)
        - f32::from(pad.buttons & LB != 0) - key(KeyCode::KeyQ);
    let fast = pad.buttons & LEFT_STICK_CLICK != 0 || keys.pressed(KeyCode::ShiftLeft);
    // Fly where the camera looks, including up and down.
    let movement = camera.rotation * Vec3::new(x, 0.0, -z) + Vec3::Y * y;
    camera.translation += movement * dt * if fast { FAST_SPEED } else { SPEED };
}

fn land(fly: &mut FlyMode, physics: &crate::physics::GamePhysics, skater: &mut crate::physics::SkaterRuntime) {
    let Some(ground) = ground_below(physics.world(), fly.camera.translation) else {
        fly.say("Sem chao embaixo: voe ate cima de uma superficie");
        return;
    };
    if skater.player_input.pending_teleport().is_some() {
        return;
    }
    let forward = fly.camera.rotation * Vec3::NEG_Z;
    let (sin, cos) = forward.x.atan2(forward.z).sin_cos();
    let matrix = [
        [cos, 0., -sin, 0.],
        [0., 1., 0., 0.],
        [sin, 0., cos, 0.],
        [ground.x, ground.y + 0.15, ground.z, 0.],
    ];
    if let Err(error) = skater.player_input.request_teleport(matrix) {
        warn!("Fly mode landing: {error}");
        return;
    }
    skater.teleport_state.request_manual(matrix, false);
    fly.active = false;
    fly.release_controls = true;
    fly.notice = None;
}

/// Closest upward-facing world surface straight below `position`.
fn ground_below(world: &skate_core::physics::board_world::BoardWorld, position: Vec3) -> Option<Vec3> {
    let start = Vector3::new(position.x, position.y, position.z);
    let delta = Vector3::new(0.0, -GROUND_SEARCH, 0.0);
    let end = Vector3::new(position.x, position.y - GROUND_SEARCH, position.z);
    let mut best: Option<(f32, Vector3)> = None;
    for (_, entry) in world.line_candidates(start, end, 0.0) {
        if entry.triangle.feature.normal.y < 0.3 {
            continue;
        }
        let mut hit = TriangleLineHit { position: Vector3::ZERO, normal: Vector3::ZERO,
            fraction: 0.0, volume_parameter: [0.0; 3] };
        if triangle_segment(&mut hit, start, delta, entry.triangle.vertices, 0.0, 0.0)
            && best.is_none_or(|(fraction, _)| hit.fraction < fraction)
        {
            best = Some((hit.fraction, hit.position));
        }
    }
    best.map(|(_, p)| Vec3::new(p.x, p.y, p.z))
}

#[derive(Component)]
struct FlyLabel;

fn spawn_label(mut commands: Commands) {
    commands.spawn((
        FlyLabel,
        Text::new(""),
        TextFont { font_size: 20.0, ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, top: px(56), left: percent(50),
            margin: UiRect::left(px(-160)), width: px(320), justify_content: JustifyContent::Center, ..default() },
        TextLayout::new_with_justify(Justify::Center),
        GlobalZIndex(95),
    ));
}

fn update_label(mut fly: ResMut<FlyMode>, time: Res<Time<Real>>, mut labels: Query<&mut Text, With<FlyLabel>>) {
    let dt = time.delta_secs();
    if let Some((_, remaining)) = fly.notice.as_mut() {
        *remaining -= dt;
    }
    if fly.notice.as_ref().is_some_and(|(_, remaining)| *remaining <= 0.0) {
        fly.notice = None;
    }
    let text = match (&fly.notice, fly.active) {
        (Some((notice, _)), _) => notice.clone(),
        (None, true) => "VOO  |  LB/RB desce/sobe  |  B pousa".into(),
        (None, false) => String::new(),
    };
    for mut label in &mut labels {
        if label.0 != text {
            label.0 = text.clone();
        }
    }
}
