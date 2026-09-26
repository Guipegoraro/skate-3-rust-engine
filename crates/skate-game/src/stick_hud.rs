//! Right-stick indicator at the bottom centre: ring, centre mark, live dot and a fading trail.
//! Reads the published gameplay actions, so it shows exactly what physics and tricks receive.
use crate::hud_shapes::{centre_on, centred, disc, ring};
use bevy::prelude::*;
use std::collections::VecDeque;

const SIZE: f32 = 96.0;
const DOT: f32 = 12.0;
const TRAIL_DOT: f32 = 8.0;
const TRAIL_POINTS: usize = 14;
const TRAIL_SECONDS: f32 = 0.35;
// Right stick X/Y in the published 18-action array (native actions 67/68).
const RIGHT_X: usize = 3;
const RIGHT_Y: usize = 4;

pub(crate) struct StickHudPlugin;

#[derive(Component)]
struct StickRoot;
#[derive(Component)]
struct StickDot;
#[derive(Component)]
struct TrailDot(usize);

impl Plugin for StickHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn)
            .add_systems(Update, update);
    }
}

fn spawn(mut commands: Commands) {
    let centre = Vec2::splat(SIZE * 0.5);
    commands.spawn((
        Name::new("Right stick HUD"),
        StickRoot,
        Node {
            position_type: PositionType::Absolute,
            bottom: px(36),
            left: percent(50),
            margin: UiRect::left(px(-SIZE * 0.5)),
            width: px(SIZE),
            height: px(SIZE),
            ..default()
        },
        GlobalZIndex(90),
    )).with_children(|parent| {
        parent.spawn(centred(ring(SIZE, 2.0, Color::srgba(1.0, 1.0, 1.0, 0.55)), centre));
        parent.spawn(centred(ring(SIZE * 0.3, 1.5, Color::srgba(1.0, 1.0, 1.0, 0.35)), centre));
        for index in 0..TRAIL_POINTS {
            parent.spawn((TrailDot(index), centred(disc(TRAIL_DOT, Color::NONE), centre)));
        }
        parent.spawn((StickDot, centred(disc(DOT, Color::WHITE), centre)));
    });
}

fn update(
    time: Res<Time<Real>>,
    input: Res<crate::input::PublishedTickInput>,
    menu: Option<Res<crate::graphics_menu::Menu>>,
    replay: Res<crate::replay::Replay>,
    mut trail: Local<VecDeque<(Vec2, f32)>>,
    mut root: Query<&mut Visibility, With<StickRoot>>,
    mut dot: Query<&mut Node, (With<StickDot>, Without<TrailDot>)>,
    mut dots: Query<(&TrailDot, &mut Node, &mut BackgroundColor), Without<StickDot>>,
) {
    let visible = crate::graphics_menu::gameplay_active(menu) && !replay.active;
    for mut visibility in &mut root {
        *visibility = if visible { Visibility::Inherited } else { Visibility::Hidden };
    }
    if !visible {
        trail.clear();
        return;
    }
    let values = *input.0.actions().values();
    let stick = Vec2::new(values[RIGHT_X], values[RIGHT_Y]).clamp_length_max(1.0);
    // Screen Y grows downwards; stick up is positive.
    let reach = (SIZE - DOT) * 0.5;
    let position = Vec2::splat(SIZE * 0.5) + Vec2::new(stick.x, -stick.y) * reach;
    for mut node in &mut dot {
        centre_on(&mut node, position);
    }

    let dt = time.delta_secs();
    for point in trail.iter_mut() {
        point.1 += dt;
    }
    trail.retain(|point| point.1 < TRAIL_SECONDS);
    if stick.length() > 0.1 {
        trail.push_front((position, 0.0));
        trail.truncate(TRAIL_POINTS);
    }
    for (TrailDot(index), mut node, mut color) in &mut dots {
        match trail.get(*index) {
            Some(&(at, age)) => {
                centre_on(&mut node, at);
                let alpha = 0.6 * (1.0 - age / TRAIL_SECONDS);
                color.0 = Color::srgba(0.55, 0.85, 1.0, alpha);
            }
            None => color.0 = Color::NONE,
        }
    }
}
