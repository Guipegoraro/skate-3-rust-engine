//! Reusable pause-menu option pages (SK-051): a page is a `&[OptionRow<T>]` over one settings
//! struct plus a trailing "Back" row. Used by Game options (`game_options::ROWS`) and
//! Video effects (`video_effects::ROWS`); a new page only needs its struct and rows.

/// One menu row: its label, how to show the current value, and how Left/Right/Enter change it.
pub(crate) struct OptionRow<T> {
    pub label: &'static str,
    pub value: fn(&T) -> String,
    pub change: fn(&mut T, i32),
}

/// Row count of a page including its "Back" row.
pub(crate) fn page_len<T>(rows: &[OptionRow<T>]) -> usize {
    rows.len() + 1
}

/// Menu text for row `index`; past the last option it is the "Back" row.
pub(crate) fn row_text<T>(rows: &[OptionRow<T>], index: usize, settings: &T) -> String {
    match rows.get(index) {
        Some(row) => format!("{:<22}{}", row.label, (row.value)(settings)),
        None => "Back".into(),
    }
}

/// Applies Left/Right/Enter to row `index`. Returns false for the "Back" row.
pub(crate) fn change<T>(rows: &[OptionRow<T>], index: usize, settings: &mut T, step: i32) -> bool {
    match rows.get(index) {
        Some(row) => {
            (row.change)(settings, step);
            true
        }
        None => false,
    }
}

pub(crate) fn on_off(value: bool) -> String {
    if value { "On" } else { "Off" }.into()
}

/// Left/Right step by 1. Right/Enter past `max` wraps to 0, so a mouse alone can set it.
pub(crate) fn count_step(value: u32, step: i32, max: u32) -> u32 {
    if step > 0 && value >= max {
        return 0;
    }
    (value as i32 + step.signum()).clamp(0, max as i32) as u32
}

/// Named level, 0 = `names[0]` (usually "Off"); out-of-range values show the last name.
pub(crate) fn level_name(value: u32, names: &[&str]) -> String {
    names[(value as usize).min(names.len() - 1)].into()
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Toy { on: bool, level: u32 }
    const ROWS: &[OptionRow<Toy>] = &[
        OptionRow { label: "Switch", value: |t| on_off(t.on), change: |t, _| t.on = !t.on },
        OptionRow { label: "Level", value: |t| level_name(t.level, &["Off", "Low", "High"]),
            change: |t, step| t.level = count_step(t.level, step, 2) },
    ];
    #[test]
    fn page_rows_change_and_end_with_back() {
        let mut toy = Toy { on: false, level: 0 };
        assert_eq!(page_len(ROWS), 3);
        assert!(change(ROWS, 0, &mut toy, 1));
        assert!(change(ROWS, 1, &mut toy, 1));
        assert!(!change(ROWS, 2, &mut toy, 1));
        assert!(row_text(ROWS, 0, &toy).ends_with("On"));
        assert!(row_text(ROWS, 1, &toy).ends_with("Low"));
        assert_eq!(row_text(ROWS, 2, &toy), "Back");
        assert_eq!(count_step(2, 1, 2), 0);
        assert_eq!(count_step(0, -1, 2), 0);
    }
}
