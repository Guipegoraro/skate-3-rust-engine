//! Screen-space UI primitives (rings, discs) positioned by centre, for HUD overlays.
//! Shapes are absolute nodes: spawn them under a sized parent and move them with `centre_on`.
use bevy::prelude::*;

/// Hollow circle.
pub(crate) fn ring(diameter: f32, thickness: f32, color: Color) -> (Node, BorderColor) {
    (
        Node {
            border: UiRect::all(px(thickness)),
            ..circle(diameter)
        },
        BorderColor::all(color),
    )
}

/// Filled circle.
pub(crate) fn disc(diameter: f32, color: Color) -> (Node, BackgroundColor) {
    (circle(diameter), BackgroundColor(color))
}

fn circle(diameter: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: px(diameter),
        height: px(diameter),
        border_radius: BorderRadius::MAX,
        ..default()
    }
}

/// Straight line of fixed thickness; place it with `span`.
pub(crate) fn segment(thickness: f32, color: Color) -> (Node, BackgroundColor, UiTransform) {
    (
        Node {
            position_type: PositionType::Absolute,
            width: px(0),
            height: px(thickness),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(color),
        UiTransform::IDENTITY,
    )
}

/// Stretches and rotates a `segment` so it runs from `from` to `to`, in parent pixels.
pub(crate) fn span(node: &mut Node, transform: &mut UiTransform, from: Vec2, to: Vec2) {
    let delta = to - from;
    node.width = px(delta.length());
    centre_on(node, (from + to) * 0.5);
    // UI Y points down, so atan2 in screen space is already the clockwise rotation.
    transform.rotation = Rot2::radians(delta.y.atan2(delta.x));
}

/// Moves an absolute shape so its centre sits at `centre`, in pixels from the parent's top-left.
pub(crate) fn centre_on(node: &mut Node, centre: Vec2) {
    let size = Vec2::new(px_value(node.width), px_value(node.height));
    node.left = px(centre.x - size.x * 0.5);
    node.top = px(centre.y - size.y * 0.5);
}

/// A shape bundle already centred on `centre`.
pub(crate) fn centred<C: Component>((mut node, style): (Node, C), centre: Vec2) -> (Node, C) {
    centre_on(&mut node, centre);
    (node, style)
}

fn px_value(value: Val) -> f32 {
    match value {
        Val::Px(pixels) => pixels,
        _ => 0.0,
    }
}
