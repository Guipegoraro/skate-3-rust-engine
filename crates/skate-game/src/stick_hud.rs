//! Right-stick indicator at the bottom centre: ring, centre mark, live dot, and a line that
//! draws each flick's path. The line holds after the stick returns to centre, then fades.
//! Reads the published gameplay actions, so it shows exactly what physics and tricks receive.
use crate::hud_shapes::{centre_on, centred, disc, ring, segment, span};
use bevy::prelude::*;

const SIZE: f32 = 96.0;
const DOT: f32 = 12.0;
const LINE: f32 = 3.0;
const MAX_POINTS: usize = 64;
// Stick magnitude that starts a stroke, and the minimum pixel step between recorded points.
const ACTIVE: f32 = 0.2;
const STEP: f32 = 1.5;
const HOLD_SECONDS: f32 = 1.0;
const FADE_SECONDS: f32 = 0.5;
const LINE_COLOR: Vec3 = Vec3::new(0.55, 0.85, 1.0);
// Right stick X/Y in the published 18-action array (native actions 67/68).
const RIGHT_X: usize = 3;
const RIGHT_Y: usize = 4;

pub(crate) struct StickHudPlugin;

#[derive(Component)]
struct StickRoot;
#[derive(Component)]
struct StickDot;
#[derive(Component)]
struct LinePiece(usize);

/// The current (or last) flick path in indicator pixels.
#[derive(Default)]
struct Stroke {
    points: Vec<Vec2>,
    active: bool,
    released_for: f32,
}
impl Stroke {
    fn record(&mut self, stick: Vec2, position: Vec2, dt: f32) {
        let centre = Vec2::splat(SIZE * 0.5);
        if stick.length() > ACTIVE {
            if !self.active {
                // A new flick replaces the previous drawing and starts from the rest position.
                self.points = vec![centre];
                self.active = true;
            }
            if self.points.last().is_none_or(|last| last.distance(position) >= STEP) {
                if self.points.len() == MAX_POINTS {
                    self.points.remove(0);
                }
                self.points.push(position);
            }
        } else if self.active {
            self.active = false;
            self.released_for = 0.0;
            if self.points.len() < MAX_POINTS {
                self.points.push(centre);
            }
        } else {
            self.released_for += dt;
            if self.released_for >= HOLD_SECONDS + FADE_SECONDS {
                self.points.clear();
            }
        }
    }
    fn alpha(&self) -> f32 {
        if self.active {
            return 1.0;
        }
        1.0 - ((self.released_for - HOLD_SECONDS) / FADE_SECONDS).clamp(0.0, 1.0)
    }
}

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
        for index in 0..MAX_POINTS - 1 {
            parent.spawn((LinePiece(index), segment(LINE, Color::NONE)));
        }
        parent.spawn((StickDot, centred(disc(DOT, Color::WHITE), centre)));
    });
}

fn update(
    time: Res<Time<Real>>,
    input: Res<crate::input::PublishedTickInput>,
    options: Res<crate::game_options::GameOptions>,
    menu: Option<Res<crate::graphics_menu::Menu>>,
    replay: Res<crate::replay::Replay>,
    mut stroke: Local<Stroke>,
    mut root: Query<&mut Visibility, With<StickRoot>>,
    mut dot: Query<&mut Node, (With<StickDot>, Without<LinePiece>)>,
    mut pieces: Query<(&LinePiece, &mut Node, &mut UiTransform, &mut BackgroundColor), Without<StickDot>>,
) {
    let visible = options.stick_indicator && crate::graphics_menu::gameplay_active(menu) && !replay.active;
    for mut visibility in &mut root {
        *visibility = if visible { Visibility::Inherited } else { Visibility::Hidden };
    }
    if !visible {
        *stroke = Stroke::default();
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

    stroke.record(stick, position, time.delta_secs());
    let color = Color::srgba(LINE_COLOR.x, LINE_COLOR.y, LINE_COLOR.z, 0.9 * stroke.alpha());
    for (LinePiece(index), mut node, mut transform, mut background) in &mut pieces {
        match (stroke.points.get(*index), stroke.points.get(index + 1)) {
            (Some(&from), Some(&to)) => {
                node.display = Display::Flex;
                span(&mut node, &mut transform, from, to);
                background.0 = color;
            }
            _ => node.display = Display::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stroke_starts_at_centre_holds_then_clears() {
        let centre = Vec2::splat(SIZE * 0.5);
        let mut stroke = Stroke::default();
        stroke.record(Vec2::new(0.0, -1.0), centre + Vec2::new(0.0, 40.0), 0.016);
        stroke.record(Vec2::new(1.0, 0.0), centre + Vec2::new(40.0, 0.0), 0.016);
        assert_eq!(stroke.points.len(), 3);
        assert_eq!(stroke.points[0], centre);
        stroke.record(Vec2::ZERO, centre, 0.016);
        assert_eq!(*stroke.points.last().unwrap(), centre);
        stroke.record(Vec2::ZERO, centre, HOLD_SECONDS * 0.5);
        assert_eq!(stroke.alpha(), 1.0);
        stroke.record(Vec2::ZERO, centre, HOLD_SECONDS + FADE_SECONDS);
        assert!(stroke.points.is_empty());
    }
}
