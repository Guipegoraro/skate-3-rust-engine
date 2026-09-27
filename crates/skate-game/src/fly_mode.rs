//! On-foot fly/noclip mode (Game options "Fly mode"). B (or F3) on foot freezes the skater's
//! native physics, as a mod vehicle does, and hands the controller to a free camera. Pressing
//! it again teleports the skater on foot onto the ground below the camera, through the same
//! manual teleport path the vehicle exit uses; Y (or F4) drops the skater right where the camera
//! is instead. D-pad or arrow keys up/down change the flight speed. Disabled in multiplayer.
use crate::camera::GameplayCamera;
use bevy::prelude::*;

const DPAD_UP: u16 = 0x0001;
const DPAD_DOWN: u16 = 0x0002;
const B: u16 = 0x2000;
const Y: u16 = 0x8000;
const LB: u16 = 0x0100;
const RB: u16 = 0x0200;
const LEFT_STICK_CLICK: u16 = 0x0040;
const SPEED: f32 = 8.0;
const FAST_SPEED: f32 = 32.0;
const GROUND_SEARCH: f32 = 500.0;
/// Speed multiplier steps (D-pad/arrow up and down), from 0.25x to 16x.
const SPEED_STEP: f32 = 1.5;
const SPEED_RANGE: (f32, f32) = (0.25, 16.0);
/// Camera eye height above the skater's feet when dropping at the camera.
const EYE_HEIGHT: f32 = 1.6;
// On-foot physical category (BipedGround, BipedAir, pushing, landing on deck).
const ON_FOOT: u32 = 500;

#[derive(Resource, Default)]
pub(crate) struct FlyMode {
    active: bool,
    camera: Transform,
    previous_buttons: u16,
    release_controls: bool,
    notice: Option<(String, f32)>,
    speed: f32,
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
    fn speed(&self) -> f32 {
        if self.speed > 0.0 { self.speed } else { 1.0 }
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
    let drop_here = !menu_open && fly.active && (pressed & Y != 0 || keys.just_pressed(KeyCode::F4));
    if drop_here {
        let feet = fly.camera.translation - Vec3::Y * EYE_HEIGHT;
        place(&mut fly, &mut skater, feet);
    } else if toggle && fly.active {
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
                fly.say("Voo ligado: B pousa no chao, Y solta aqui");
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
    let faster = pressed & DPAD_UP != 0 || keys.just_pressed(KeyCode::ArrowUp);
    let slower = pressed & DPAD_DOWN != 0 || keys.just_pressed(KeyCode::ArrowDown);
    if faster || slower {
        let step = if faster { SPEED_STEP } else { SPEED_STEP.recip() };
        fly.speed = (fly.speed() * step).clamp(SPEED_RANGE.0, SPEED_RANGE.1);
    }
    let speed = fly.speed();
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
    camera.translation += movement * dt * speed * if fast { FAST_SPEED } else { SPEED };
}

fn land(fly: &mut FlyMode, physics: &crate::physics::GamePhysics, skater: &mut crate::physics::SkaterRuntime) {
    let Some(ground) = ground_below(physics.world(), fly.camera.translation) else {
        fly.say("Sem chao embaixo: voe ate cima de uma superficie");
        return;
    };
    place(fly, skater, ground + Vec3::Y * 0.15);
}

/// Ends the flight with the skater on foot at `feet`, facing where the camera looks.
fn place(fly: &mut FlyMode, skater: &mut crate::physics::SkaterRuntime, feet: Vec3) {
    if skater.player_input.pending_teleport().is_some() {
        return;
    }
    let forward = fly.camera.rotation * Vec3::NEG_Z;
    let (sin, cos) = forward.x.atan2(forward.z).sin_cos();
    let matrix = [
        [cos, 0., -sin, 0.],
        [0., 1., 0., 0.],
        [sin, 0., cos, 0.],
        [feet.x, feet.y, feet.z, 0.],
    ];
    if let Err(error) = skater.player_input.request_teleport(matrix) {
        warn!("Fly mode exit: {error}");
        return;
    }
    skater.teleport_state.request_manual(matrix, false);
    fly.active = false;
    fly.release_controls = true;
    fly.notice = None;
}

/// Closest upward-facing world surface straight below `position`.
fn ground_below(world: &skate_core::physics::board_world::BoardWorld, position: Vec3) -> Option<Vec3> {
    crate::world_probe::ground_below(world, position, GROUND_SEARCH, 0.3)
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
            margin: UiRect::left(px(-380)), width: px(760), justify_content: JustifyContent::Center, ..default() },
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
        (None, true) => format!("VOO {:.2}x  |  setas velocidade  |  LB/RB desce/sobe  |  B pousa  |  Y solta aqui", fly.speed()),
        (None, false) => String::new(),
    };
    for mut label in &mut labels {
        if label.0 != text {
            label.0 = text.clone();
        }
    }
}
