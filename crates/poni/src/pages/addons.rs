//! Add-ons page, the "+" at the end of the tab row: features only the people
//! who switch them on get (`poni_core::addons`). All are built in; nothing
//! downloads.
//!
//! One row per add-on: its icon, its name, what it does, "Built in" while it
//! is off, and a switch. The whole row toggles, as in Settings
//! (`Command::SaveSettings`).

use crate::commands::Command;
use crate::gfx::TextStyle;
use crate::icons::Icon;
use crate::state::AppState;
use crate::ui::{wid_n, Ui};
use poni_core::addons::{self, Addon, Status};
use poni_core::geom::{palette, Color, Point, Rect};

const INTRO_H: f32 = 18.0;
const ROWS_Y: f32 = 28.0;
const ROW_H: f32 = 52.0;
const BADGE: f32 = 36.0;
const TEXT_X: f32 = 48.0;
/// A row's hover wash reaches this far past its contents (as in Settings).
const ROW_BLEED: f32 = 8.0;
const SWITCH_W: f32 = 30.0;
const SWITCH_H: f32 = 18.0;

/// The add-ons this app has, in the page's order.
pub const OFFERED: &[&Addon] = &addons::ALL;

pub fn height(_st: &AppState) -> f32 {
    ROWS_Y + OFFERED.len() as f32 * ROW_H
}

/// Each add-on's icon on its badge.
fn icon(a: &Addon) -> Icon {
    match a.id {
        "script" => Icon::Script,
        "outlook" => Icon::Agenda,
        _ => Icon::Addons,
    }
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let intro = "Each one stays off until you switch it on.";
    ui.c.text(intro, Rect::new(area.x, area.y, area.w, INTRO_H), &TextStyle::footnote().color(palette::TEXT_DIM));
    // The last row's hover wash meets the Pill card's rounded corners.
    let card = super::clipboard::pill_card(st, area);
    for (i, a) in OFFERED.iter().enumerate() {
        let row = Rect::new(area.x, area.y + ROWS_Y + i as f32 * ROW_H, area.w, ROW_H);
        if addon_row(ui, st, row, i, a, card) {
            let on = st.settings.addon_on(a.id);
            st.settings.set_addon(a.id, !on);
            st.cmd(Command::SaveSettings);
        }
    }
}

/// Returns true when the row was clicked. `card`: the Pill card (`clipboard::pill_card`).
fn addon_row(ui: &mut Ui, st: &AppState, r: Rect, i: usize, a: &Addon, card: Option<(Rect, f32)>) -> bool {
    let status = st.addon(a.id);
    let accent = st.accent;
    let hit = Rect::new(r.x - ROW_BLEED, r.y + 2.0, r.w + 2.0 * ROW_BLEED, r.h - 4.0);
    let resp = ui.button(wid_n("addons.row", i as u64), hit);
    if resp.hovered || resp.pressed {
        super::clipboard::fill_box(ui, card, hit, 10.0, Color::white(if resp.pressed { 0.10 } else { 0.06 }));
    }
    let cy = r.center().y;
    ui.c.fill_circle(Point::new(r.x + BADGE / 2.0, cy), BADGE / 2.0, accent.with_alpha(0.18));
    ui.c.icon(icon(a), Rect::from_center(Point::new(r.x + BADGE / 2.0, cy), 18.0, 18.0), accent);

    let sw = Rect::new(r.right() - SWITCH_W, cy - SWITCH_H / 2.0, SWITCH_W, SWITCH_H);
    crate::pages::settings::draw_switch(ui.c, sw, status.is_on(), accent);
    // "Built in", while there is a choice to make.
    let mut text_right = sw.x - 16.0;
    if status == Status::Off {
        let size = "Built in";
        let style = TextStyle::footnote().color(palette::TEXT_DIM).right();
        let w = ui.c.measure(size, &style, 200.0).0.ceil();
        ui.c.text(size, Rect::new(sw.x - 10.0 - w, r.y, w, r.h), &style);
        text_right = sw.x - 10.0 - w - 16.0;
    }

    let x = r.x + TEXT_X;
    let w = (text_right - x).max(0.0);
    ui.c.text(a.title, Rect::new(x, r.y + 6.0, w, 20.0), &TextStyle::headline());
    ui.c.text(&status.line(a), Rect::new(x, r.y + 27.0, w, 17.0), &TextStyle::footnote().color(palette::TEXT_DIM));
    resp.clicked
}
