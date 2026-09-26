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
// Recognized trick (SK-027): the pattern's ideal path in amber, plus the trick name.
const IDEAL_COLOR: Vec3 = Vec3::new(1.0, 0.72, 0.2);
const IDEAL_POINTS: usize = 16;
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
#[derive(Component)]
struct IdealPiece(usize);
#[derive(Component)]
struct TrickName;

/// Last recognition shown: its count and seconds since it arrived.
#[derive(Default)]
struct Ideal {
    count: u64,
    age: f32,
    points: Vec<Vec2>,
}
impl Ideal {
    fn alpha(&self) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        1.0 - ((self.age - HOLD_SECONDS) / FADE_SECONDS).clamp(0.0, 1.0)
    }
}

/// Pattern key points (stick units, y down) to indicator pixels, starting from rest.
fn ideal_path(points: &[[f32; 2]]) -> Vec<Vec2> {
    let centre = Vec2::splat(SIZE * 0.5);
    let reach = (SIZE - DOT) * 0.5;
    std::iter::once(centre)
        .chain(points.iter().map(|&[x, y]| centre + Vec2::new(x, y).clamp_length_max(1.0) * reach))
        .take(IDEAL_POINTS)
        .collect()
}

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

// Original Trick Analyzer ring: the texture is the left half; the right half is its mirror.
const RING_TEXTURE: &str = "hud2/trickanalyser/1";

fn spawn(
    mut commands: Commands,
    mut ui: ResMut<crate::ui_textures::UiTextures>,
    mut images: ResMut<Assets<Image>>,
) {
    let centre = Vec2::splat(SIZE * 0.5);
    let ring_texture = ui.get(RING_TEXTURE, &mut images);
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
        match ring_texture {
            Some(texture) => {
                for (left, flip_x) in [(0.0, false), (SIZE * 0.5, true)] {
                    parent.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(left),
                            top: px(0),
                            width: px(SIZE * 0.5),
                            height: px(SIZE),
                            ..default()
                        },
                        ImageNode { flip_x, ..ImageNode::new(texture.clone()) },
                    ));
                }
            }
            None => {
                parent.spawn(centred(ring(SIZE, 2.0, Color::srgba(1.0, 1.0, 1.0, 0.55)), centre));
                parent.spawn(centred(ring(SIZE * 0.3, 1.5, Color::srgba(1.0, 1.0, 1.0, 0.35)), centre));
            }
        }
        for index in 0..IDEAL_POINTS - 1 {
            parent.spawn((IdealPiece(index), segment(LINE + 1.0, Color::NONE)));
        }
        for index in 0..MAX_POINTS - 1 {
            parent.spawn((LinePiece(index), segment(LINE, Color::NONE)));
        }
        parent.spawn((
            TrickName,
            Text::new(""),
            TextFont { font_size: 18.0, ..default() },
            TextColor(Color::NONE),
            TextLayout::new_with_justify(Justify::Center),
            Node {
                position_type: PositionType::Absolute,
                bottom: px(SIZE + 4.0),
                left: px(-SIZE),
                width: px(SIZE * 3.0),
                ..default()
            },
        ));
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
    mut ideal: Local<Ideal>,
    controls: Res<crate::physics::PlayerControls>,
    mut root: Query<&mut Visibility, With<StickRoot>>,
    mut dot: Query<&mut Node, (With<StickDot>, Without<LinePiece>, Without<IdealPiece>, Without<TrickName>)>,
    mut pieces: Query<(&LinePiece, &mut Node, &mut UiTransform, &mut BackgroundColor), (Without<StickDot>, Without<IdealPiece>)>,
    mut ideal_pieces: Query<(&IdealPiece, &mut Node, &mut UiTransform, &mut BackgroundColor), (Without<StickDot>, Without<LinePiece>)>,
    mut name: Query<(&mut Text, &mut TextColor), With<TrickName>>,
) {
    let visible = options.stick_indicator && crate::graphics_menu::gameplay_active(menu) && !replay.active;
    for mut visibility in &mut root {
        *visibility = if visible { Visibility::Inherited } else { Visibility::Hidden };
    }
    if !visible {
        *stroke = Stroke::default();
        ideal.points.clear();
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

    ideal.age += time.delta_secs();
    if let Some(recognized) = controls.recognized_gesture() {
        if recognized.count != ideal.count {
            ideal.count = recognized.count;
            ideal.age = 0.0;
            ideal.points = ideal_path(&recognized.points);
            for (mut text, _) in &mut name {
                text.0 = recognized.name.replace('_', " ");
            }
        }
    }
    let alpha = ideal.alpha();
    if alpha <= 0.0 {
        ideal.points.clear();
    }
    let amber = Color::srgba(IDEAL_COLOR.x, IDEAL_COLOR.y, IDEAL_COLOR.z, 0.9 * alpha);
    for (_, mut color) in &mut name {
        color.0 = amber;
    }
    for (IdealPiece(index), mut node, mut transform, mut background) in &mut ideal_pieces {
        match (ideal.points.get(*index), ideal.points.get(index + 1)) {
            (Some(&from), Some(&to)) => {
                node.display = Display::Flex;
                span(&mut node, &mut transform, from, to);
                background.0 = amber;
            }
            _ => node.display = Display::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ideal_path_starts_at_rest_and_fades_with_the_stroke() {
        let centre = Vec2::splat(SIZE * 0.5);
        let reach = (SIZE - DOT) * 0.5;
        let path = ideal_path(&[[0.0, 1.0], [1.0, -1.0]]);
        assert_eq!(path[0], centre);
        assert_eq!(path[1], centre + Vec2::new(0.0, reach));
        // Diagonal key points stay on the ring.
        assert!((path[2].distance(centre) - reach).abs() < 1e-3);
        let mut ideal = Ideal { count: 1, age: 0.0, points: path };
        assert_eq!(ideal.alpha(), 1.0);
        ideal.age = HOLD_SECONDS + FADE_SECONDS;
        assert_eq!(ideal.alpha(), 0.0);
    }
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
