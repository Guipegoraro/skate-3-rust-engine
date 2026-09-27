//! Menu sounds (SK-022): cursor changes become `menu_open`, `menu_close` and `menu_move`
//! events (ids from the original pause menu, traced in Skate3Recomp). Any menu can feed
//! `menu_cue` with its own cursor; the pause menu is the first.
use super::PlaySound;
use bevy::prelude::*;

/// Where a menu is: open or not, which page and which row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MenuCursor {
    pub open: bool,
    pub page: usize,
    pub row: usize,
}

/// The sound event for a cursor change, if any.
pub(crate) fn menu_cue(from: MenuCursor, to: MenuCursor) -> Option<&'static str> {
    match (from.open, to.open) {
        (false, true) => Some("menu_open"),
        (true, false) => Some("menu_close"),
        (true, true) if from != to => Some("menu_move"),
        _ => None,
    }
}

pub(super) fn pause_menu_sounds(
    menu: Option<Res<crate::graphics_menu::Menu>>,
    mut previous: Local<Option<MenuCursor>>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let Some(menu) = menu else { return };
    let cursor = menu.sound_cursor();
    if let Some(from) = previous.replace(cursor) {
        if let Some(event) = menu_cue(from, cursor) {
            sounds.write(PlaySound(event.into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_changes_map_to_menu_events() {
        let closed = MenuCursor::default();
        let open = MenuCursor { open: true, ..closed };
        assert_eq!(menu_cue(closed, open), Some("menu_open"));
        assert_eq!(menu_cue(open, closed), Some("menu_close"));
        assert_eq!(menu_cue(open, MenuCursor { row: 1, ..open }), Some("menu_move"));
        assert_eq!(menu_cue(open, MenuCursor { page: 1, ..open }), Some("menu_move"));
        assert_eq!(menu_cue(open, open), None);
        assert_eq!(menu_cue(closed, MenuCursor { row: 3, ..closed }), None, "closed menus stay silent");
    }
}
