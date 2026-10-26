//! Notes page: quick notes, newest first. The input is the first row (Enter
//! saves and keeps the field for the next one, Esc leaves it); each note
//! shows when it was written (the time today, else the day). A click copies
//! a note (it doesn't land in the Clipboard page's history), and the × on a
//! hovered row deletes it, with Undo for a few seconds.
//!
//! The notes live in `data.json` (`dyna_core::notes`). Nothing animates: the
//! page draws when something changes, and wakes only for the end of a
//! "Copied" flash or an Undo lapsing (`next_wake`).

use crate::commands::Command;
use crate::gfx::TextStyle;
use crate::icons::Icon;
use crate::state::AppState;
use crate::ui::{wid, wid_n, PillStyle, Ui, PILL_SMALL};
use dyna_core::geom::{palette, Color, Point, Rect};
use dyna_core::notes::{when, MAX_NOTE_LEN};

/// Tasks' height and rhythm: the input, four rows, a footer.
const HEIGHT: f32 = 200.0;
const INPUT_H: f32 = 34.0;
const LIST_TOP: f32 = 44.0;
const ROW_H: f32 = 31.0;
const LIST_H: f32 = 4.0 * ROW_H;
const FOOTER_H: f32 = PILL_SMALL;
const TEXT_X: f32 = 36.0;
/// The time column and the delete button on the right of a row.
const WHEN_W: f32 = 56.0;
const DELETE_D: f32 = 22.0;
const WHEEL_STEP: f32 = ROW_H * 1.5;
const FLASH_S: f64 = 1.2;
const UNDO_S: f64 = 6.0;

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

/// The end of a "Copied" flash or of the Undo offer, whichever comes first.
pub fn next_wake(st: &AppState) -> Option<f64> {
    let flash = st.ui.note_flash.map(|(_, t0)| t0 + FLASH_S);
    let undo = st.ui.note_undo.as_ref().map(|(_, _, t0)| t0 + UNDO_S);
    [flash, undo].into_iter().flatten().filter(|t| *t > st.now).reduce(f64::min)
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let now = st.now;
    if st.ui.note_flash.is_some_and(|(_, t0)| now - t0 >= FLASH_S) {
        st.ui.note_flash = None;
    }
    let undo_id = wid("notes.undo");
    let undo_held = ui.input.pressed_id == Some(undo_id) || ui.input.clicked.is_some_and(|(id, _)| id == undo_id);
    if st.ui.note_undo.as_ref().is_some_and(|(_, _, t0)| now - t0 >= UNDO_S) && !undo_held {
        st.ui.note_undo = None;
    }

    // ---- Input.
    let accent = st.accent;
    let input_r = Rect::new(area.x, area.y, area.w, INPUT_H);
    let resp = ui.text_input_row(wid("notes.input"), input_r, &mut st.ui.note_input, "Write a note, Enter to save", MAX_NOTE_LEN, accent, Some(Icon::Plus));
    if resp.submitted {
        let (day, time) = (st.local.day_key(), dyna_core::datetime::fmt_clock(&st.local, st.settings.use_24h));
        if st.store.notes.add(&st.ui.note_input, &day, &time).is_some() {
            st.ui.note_scroll = 0.0;
            st.cmd(Command::SaveStore);
        }
        st.ui.note_input.clear();
    }

    // ---- List.
    let view = Rect::new(area.x, area.y + LIST_TOP, area.w, LIST_H);
    let footer = Rect::new(area.x, area.y + HEIGHT - FOOTER_H, area.w, FOOTER_H);
    let n = st.store.notes.items.len();
    if n == 0 {
        let hint = Rect::new(area.x, view.y, area.w, HEIGHT - LIST_TOP - FOOTER_H);
        super::empty_state(ui, hint, Icon::Note, "No notes yet", "For a number on a call, a name, a thought for later");
    } else {
        let max_scroll = (n as f32 * ROW_H - LIST_H).max(0.0);
        if max_scroll > 0.0 {
            st.ui.note_scroll -= ui.wheel_in(view) * WHEEL_STEP;
        }
        st.ui.note_scroll = st.ui.note_scroll.clamp(0.0, max_scroll);
        let today = st.local.day_key();
        let first = (st.ui.note_scroll / ROW_H).floor() as usize;
        let card = super::clipboard::pill_card(st, area);
        let mut copy = None;
        let mut delete = None;
        ui.c.clip(view);
        for i in first..n.min(first + 6) {
            let y = view.y + i as f32 * ROW_H - st.ui.note_scroll;
            let row = Rect::new(view.x, y, view.w, ROW_H);
            if row.bottom() <= view.y || row.y >= view.bottom() {
                continue;
            }
            let note = &st.store.notes.items[i];
            let r = ui.button(wid_n("notes.row", note.id), row.inset(0.0, 1.0));
            if r.hovered || r.pressed {
                super::clipboard::fill_box(ui, card, row.inset(0.0, 1.0), 8.0, Color::white(if r.pressed { 0.10 } else { 0.06 }));
            }
            ui.c.icon(Icon::Note, Rect::from_center(Point::new(row.x + 18.0, row.center().y), 14.0, 14.0), palette::TEXT_FAINT);
            let text_r = Rect::new(row.x + TEXT_X, row.y, (row.w - TEXT_X - WHEN_W - DELETE_D - 12.0).max(0.0), row.h);
            ui.c.text(&note.text, text_r, &TextStyle::body());
            let when_r = Rect::new(row.right() - DELETE_D - 8.0 - WHEN_W, row.y, WHEN_W, row.h);
            let flashing = st.ui.note_flash.is_some_and(|(id, _)| id == note.id);
            let (label, color) = if flashing { ("Copied".to_string(), accent) } else { (when(note, &today), palette::TEXT_FAINT) };
            ui.c.text(&label, when_r, &TextStyle::footnote().tabular().color(color).right());
            if r.hovered || ui.hovered(delete_rect(row)) {
                let d = ui.icon_button(wid_n("notes.delete", note.id), delete_rect(row), Icon::Close, palette::TEXT_DIM, 11.0);
                if d.clicked {
                    delete = Some(note.id);
                }
            }
            if r.clicked && delete.is_none() {
                copy = Some((note.id, note.text.clone()));
            }
        }
        ui.c.unclip();
        if let Some(id) = delete {
            if let Some((at, note)) = st.store.notes.remove(id) {
                st.ui.note_undo = Some((at, note, now));
                st.cmd(Command::SaveStore);
            }
        } else if let Some((id, text)) = copy {
            st.cmd(Command::CopyText(text));
            st.ui.note_flash = Some((id, now));
        }
    }

    // ---- Footer: how many, or Undo after a delete.
    if st.ui.note_undo.is_some() {
        let label = "Undo";
        let w = ui.pill_width(label, false, true);
        let b = Rect::new(footer.x, footer.y, w, FOOTER_H);
        if ui.pill(undo_id, b, label, None, PillStyle::UtilityAccent(accent)).clicked {
            if let Some((at, note, _)) = st.ui.note_undo.take() {
                st.store.notes.restore(at, note);
                st.cmd(Command::SaveStore);
            }
        }
        ui.c.text("Note deleted", Rect::new(b.right() + 10.0, footer.y, 200.0, footer.h), &TextStyle::footnote().color(palette::TEXT_DIM));
    } else if n > 0 {
        let count = if n == 1 { "1 note".to_string() } else { format!("{n} notes") };
        ui.c.text(&count, footer, &TextStyle::footnote().color(palette::TEXT_FAINT));
    }
}

fn delete_rect(row: Rect) -> Rect {
    Rect::from_center(Point::new(row.right() - DELETE_D / 2.0 - 4.0, row.center().y), DELETE_D, DELETE_D)
}
